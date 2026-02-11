use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use futures::{StreamExt, future::ready};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use workspace_utils::{msg_store::MsgStore, path::make_path_relative};

use crate::logs::{
    ActionType, CommandExitStatus, CommandRunResult, FileChange, NormalizedEntry,
    NormalizedEntryError, NormalizedEntryType, ToolStatus,
    plain_text_processor::PlainTextLogProcessor,
    utils::{
        EntryIndexProvider,
        patch::{add_normalized_entry, replace_normalized_entry},
    },
};

pub fn normalize_logs(
    msg_store: Arc<MsgStore>,
    worktree_path: &Path,
    entry_index_provider: EntryIndexProvider,
) {
    normalize_stderr_logs(msg_store.clone(), entry_index_provider.clone());

    let worktree_path = worktree_path.to_path_buf();
    // Record the process start time to filter out session files from concurrent runs
    let process_start_time = SystemTime::now();

    tokio::spawn(async move {
        let session_id_pushed = Arc::new(AtomicBool::new(false));
        let session_discovery_in_flight = Arc::new(AtomicBool::new(false));
        let mut tool_states: HashMap<String, ToolState> = HashMap::new();
        let mut current_message_content = String::new();
        let mut current_thinking_content = String::new();
        let mut thinking_entry_index: Option<usize> = None;

        let worktree_path_str = worktree_path.to_string_lossy();

        let mut lines_stream = msg_store
            .stdout_lines_stream()
            .filter_map(|res| ready(res.ok()));

        while let Some(line) = lines_stream.next().await {
            let trimmed = line.trim();

            // Parse Pi RPC event
            let pi_event = match serde_json::from_str::<PiEvent>(trimmed) {
                Ok(event) => event,
                Err(_) => {
                    // Handle non-JSON output as raw system message
                    if !trimmed.is_empty() {
                        let entry = NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::SystemMessage,
                            content: strip_ansi_escapes::strip_str(trimmed).to_string(),
                            metadata: None,
                        };
                        add_normalized_entry(&msg_store, &entry_index_provider, entry);
                    }
                    continue;
                }
            };

            // Handle different Pi event types
            match pi_event {
                PiEvent::AgentStart { session_id, model } => {
                    tracing::debug!("Pi AgentStart event received: session_id={:?}", session_id);

                    if !session_id_pushed.load(Ordering::Relaxed) {
                        if let Some(sid) = session_id {
                            push_session_id_once(&msg_store, &session_id_pushed, sid);
                        } else {
                            spawn_session_discovery(
                                msg_store.clone(),
                                worktree_path.clone(),
                                session_id_pushed.clone(),
                                session_discovery_in_flight.clone(),
                                process_start_time,
                            );
                        }
                    }

                    if let Some(model_name) = model {
                        let entry = NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::SystemMessage,
                            content: format!("model: {}", model_name),
                            metadata: None,
                        };
                        add_normalized_entry(&msg_store, &entry_index_provider, entry);
                    }
                }

                PiEvent::MessageUpdate {
                    message: _,
                    assistant_message_event,
                } => {
                    match assistant_message_event {
                        // --- Thinking events ---
                        AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                            current_thinking_content.push_str(&delta);

                            let entry = NormalizedEntry {
                                timestamp: None,
                                entry_type: NormalizedEntryType::Thinking,
                                content: current_thinking_content.clone(),
                                metadata: None,
                            };

                            if let Some(idx) = thinking_entry_index {
                                replace_normalized_entry(&msg_store, idx, entry);
                            } else {
                                let idx = add_normalized_entry(
                                    &msg_store,
                                    &entry_index_provider,
                                    entry,
                                );
                                thinking_entry_index = Some(idx);
                            }
                        }
                        AssistantMessageEvent::ThinkingEnd { content, .. } => {
                            if !content.is_empty() {
                                current_thinking_content = content;
                            }

                            let entry = NormalizedEntry {
                                timestamp: None,
                                entry_type: NormalizedEntryType::Thinking,
                                content: current_thinking_content.clone(),
                                metadata: None,
                            };

                            if let Some(idx) = thinking_entry_index {
                                replace_normalized_entry(&msg_store, idx, entry);
                            } else {
                                add_normalized_entry(&msg_store, &entry_index_provider, entry);
                            }

                            // Reset for potential next thinking block
                            current_thinking_content.clear();
                            thinking_entry_index = None;
                        }

                        // --- Text events ---
                        AssistantMessageEvent::TextDelta { delta, .. } => {
                            current_message_content.push_str(&delta);
                        }
                        AssistantMessageEvent::TextEnd { content, .. } => {
                            if !content.is_empty() {
                                current_message_content = content;
                            }
                        }

                        _ => {
                            // ThinkingStart, TextStart, Toolcall* events are
                            // structural markers and don't carry content we need.
                        }
                    }
                }

                PiEvent::ToolExecutionStart {
                    tool_call_id,
                    tool_name,
                    args,
                } => {
                    let tool_state = create_tool_state(&tool_name, &args, &worktree_path_str);
                    if let Some(state) = tool_state {
                        let index = add_normalized_entry(
                            &msg_store,
                            &entry_index_provider,
                            state.to_normalized_entry(),
                        );
                        tool_states.insert(
                            tool_call_id,
                            ToolState {
                                index: Some(index),
                                ..state
                            },
                        );
                    }
                }

                PiEvent::ToolExecutionEnd {
                    tool_call_id,
                    result,
                    is_error,
                    ..
                } => {
                    if let Some(mut state) = tool_states.remove(&tool_call_id) {
                        state.status = if is_error.unwrap_or(false) {
                            ToolStatus::Failed
                        } else {
                            ToolStatus::Success
                        };

                        // Update state with output if needed
                        update_tool_state_with_output(&mut state, result);

                        if let Some(index) = state.index {
                            replace_normalized_entry(
                                &msg_store,
                                index,
                                state.to_normalized_entry(),
                            );
                        }
                    }
                }

                PiEvent::TurnEnd { .. } | PiEvent::AgentEnd { .. } => {
                    // Flush any accumulated message content
                    if !current_message_content.is_empty() {
                        let entry = NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::AssistantMessage,
                            content: current_message_content.clone(),
                            metadata: None,
                        };
                        add_normalized_entry(&msg_store, &entry_index_provider, entry);
                        current_message_content.clear();
                    }
                }

                PiEvent::Error { error } => {
                    let entry = NormalizedEntry {
                        timestamp: None,
                        entry_type: NormalizedEntryType::ErrorMessage {
                            error_type: NormalizedEntryError::Other,
                        },
                        content: error,
                        metadata: None,
                    };
                    add_normalized_entry(&msg_store, &entry_index_provider, entry);
                }

                PiEvent::Response {
                    command,
                    success,
                    data,
                    error,
                    ..
                } => {
                    // Try to extract session_id from get_state responses
                    if command == "get_state" && success {
                        if let Some(session_id) = extract_session_id_from_state(&data) {
                            push_session_id_once(
                                &msg_store,
                                &session_id_pushed,
                                session_id.to_string(),
                            );
                        }
                    }

                    // RPC responses confirm receipt of commands (e.g. the initial prompt).
                    // Surface failures as error messages so they're visible to the user.
                    if !success {
                        let msg = error.unwrap_or_else(|| {
                            format!("RPC command '{}' failed (no details)", command)
                        });
                        let entry = NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ErrorMessage {
                                error_type: NormalizedEntryError::Other,
                            },
                            content: msg,
                            metadata: None,
                        };
                        add_normalized_entry(&msg_store, &entry_index_provider, entry);
                    }
                }

                _ => {
                    // Forward-compatible: MessageStart, MessageEnd, TurnStart,
                    // ToolExecutionUpdate, and any future event types are
                    // silently ignored.
                }
            }
        }

        // Flush any remaining content when the stream ends.
        // This covers cases where the process exits or crashes before
        // emitting a TurnEnd/AgentEnd event.
        if !current_thinking_content.is_empty() {
            let entry = NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::Thinking,
                content: current_thinking_content,
                metadata: None,
            };
            if let Some(idx) = thinking_entry_index {
                replace_normalized_entry(&msg_store, idx, entry);
            } else {
                add_normalized_entry(&msg_store, &entry_index_provider, entry);
            }
        }

        if !current_message_content.is_empty() {
            let entry = NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::AssistantMessage,
                content: current_message_content,
                metadata: None,
            };
            add_normalized_entry(&msg_store, &entry_index_provider, entry);
        }
    });
}

fn normalize_stderr_logs(msg_store: Arc<MsgStore>, entry_index_provider: EntryIndexProvider) {
    tokio::spawn(async move {
        let mut stderr = msg_store.stderr_chunked_stream();

        let mut processor = PlainTextLogProcessor::builder()
            .normalized_entry_producer(Box::new(|content: String| NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::ErrorMessage {
                    error_type: NormalizedEntryError::Other,
                },
                content,
                metadata: None,
            }))
            .transform_lines(Box::new(|lines| {
                lines.iter_mut().for_each(|line| {
                    *line = strip_ansi_escapes::strip_str(&line);
                });
            }))
            .time_gap(std::time::Duration::from_secs(2))
            .index_provider(entry_index_provider)
            .build();

        while let Some(Ok(chunk)) = stderr.next().await {
            for patch in processor.process(chunk) {
                msg_store.push_patch(patch);
            }
        }
    });
}

fn push_session_id_once(
    msg_store: &Arc<MsgStore>,
    session_id_pushed: &Arc<AtomicBool>,
    session_id: String,
) -> bool {
    if session_id_pushed.swap(true, Ordering::SeqCst) {
        return false;
    }

    tracing::info!("Pushing session_id to MsgStore: {}", session_id);
    msg_store.push_session_id(session_id);
    tracing::info!("Session ID queued for database persistence");
    true
}

/// Extract session_id from a Pi `get_state` response payload.
/// Kept `pub(crate)` so the Pi executor can reuse the same parsing logic
/// when it watches stdout for `get_state` responses.
pub(crate) fn extract_session_id_from_state(data: &Option<Value>) -> Option<String> {
    let data = data.as_ref()?;

    data.get("sessionId")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            data.get("session_id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
        .or_else(|| {
            data.pointer("/session/sessionId")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
        .or_else(|| {
            data.pointer("/session/session_id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

fn spawn_session_discovery(
    msg_store: Arc<MsgStore>,
    worktree_path: PathBuf,
    session_id_pushed: Arc<AtomicBool>,
    session_discovery_in_flight: Arc<AtomicBool>,
    process_start_time: SystemTime,
) {
    if session_id_pushed.load(Ordering::Relaxed) {
        return;
    }

    if session_discovery_in_flight.swap(true, Ordering::SeqCst) {
        return;
    }

    tracing::info!(
        "Pi session_id not in RPC output, attempting filesystem discovery from: {}",
        worktree_path.display()
    );

    tokio::spawn(async move {
        // Try up to 6 times with increasing delays: 0ms, 300ms, 600ms, 1000ms, 1500ms, 2000ms
        // Pi creates the session directory immediately but writes the file asynchronously
        // Total max delay: ~5.4 seconds to allow Pi time to write the session file
        let delays = [0, 300, 600, 1000, 1500, 2000];
        let mut discovered: Option<String> = None;

        for (attempt, &delay_ms) in delays.iter().enumerate() {
            if session_id_pushed.load(Ordering::Relaxed) {
                break;
            }

            if delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            if session_id_pushed.load(Ordering::Relaxed) {
                break;
            }

            let attempt_path = worktree_path.clone();
            let result = tokio::task::spawn_blocking(move || {
                crate::executors::pi::session::find_latest_session_id_with_constraint(
                    &attempt_path,
                    Some(process_start_time),
                )
            })
            .await;

            match result {
                Ok(Ok(id)) => {
                    tracing::info!(
                        "Successfully discovered session_id on attempt {}: {}",
                        attempt + 1,
                        id
                    );
                    discovered = Some(id);
                    break;
                }
                Ok(Err(e)) => {
                    if attempt < delays.len() - 1 {
                        tracing::debug!("Attempt {} failed, will retry: {}", attempt + 1, e);
                    } else {
                        tracing::warn!(
                            "Failed to discover Pi session_id from disk at {} after {} attempts: {}",
                            worktree_path.display(),
                            delays.len(),
                            e
                        );
                    }
                }
                Err(e) => {
                    if attempt < delays.len() - 1 {
                        tracing::debug!(
                            "Attempt {} failed, will retry after join error: {}",
                            attempt + 1,
                            e
                        );
                    } else {
                        tracing::warn!(
                            "Failed to discover Pi session_id from disk at {} after {} attempts: {}",
                            worktree_path.display(),
                            delays.len(),
                            e
                        );
                    }
                }
            }
        }

        if let Some(id) = discovered {
            push_session_id_once(&msg_store, &session_id_pushed, id);
        } else if !session_id_pushed.load(Ordering::Relaxed) {
            tracing::warn!(
                "No session_id available after retries - will try again on next AgentStart"
            );
        }

        session_discovery_in_flight.store(false, Ordering::SeqCst);
    });
}

/// Helper function to extract a path parameter from tool arguments.
/// Pi uses "path" as the standard parameter name, but we also check "file_path" for compatibility.
fn extract_path_param(params: &Value, tool_name: &str) -> Option<String> {
    params
        .get("path")
        .or_else(|| params.get("file_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            tracing::warn!(
                "Pi tool '{}' called without required 'path' parameter",
                tool_name
            );
            None
        })
}

/// Helper function to extract a string parameter with a warning if missing.
fn extract_string_param(params: &Value, param_name: &str, tool_name: &str) -> Option<String> {
    params
        .get(param_name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            tracing::warn!(
                "Pi tool '{}' called without required '{}' parameter",
                tool_name,
                param_name
            );
            None
        })
}

fn create_tool_state(tool: &str, params: &Value, worktree_path: &str) -> Option<ToolState> {
    match tool {
        "read" => {
            let path = extract_path_param(params, "read")?;
            let relative_path = make_path_relative(&path, worktree_path);

            Some(ToolState {
                index: None,
                tool_name: "read".to_string(),
                action_type: ActionType::FileRead {
                    path: relative_path.clone(),
                },
                status: ToolStatus::Created,
                content: relative_path,
            })
        }

        "write" => {
            let path = extract_path_param(params, "write")?;
            let content = extract_string_param(params, "content", "write").unwrap_or_default();
            let relative_path = make_path_relative(&path, worktree_path);

            Some(ToolState {
                index: None,
                tool_name: "write".to_string(),
                action_type: ActionType::FileEdit {
                    path: relative_path.clone(),
                    changes: vec![FileChange::Write {
                        content: content.to_string(),
                    }],
                },
                status: ToolStatus::Created,
                content: relative_path,
            })
        }

        "edit" => {
            let path = extract_path_param(params, "edit")?;
            let old_string = extract_string_param(params, "oldText", "edit").unwrap_or_default();
            let new_string = extract_string_param(params, "newText", "edit").unwrap_or_default();
            let relative_path = make_path_relative(&path, worktree_path);

            let diff = workspace_utils::diff::create_unified_diff(
                &relative_path,
                &old_string,
                &new_string,
            );

            Some(ToolState {
                index: None,
                tool_name: "edit".to_string(),
                action_type: ActionType::FileEdit {
                    path: relative_path.clone(),
                    changes: vec![FileChange::Edit {
                        unified_diff: diff,
                        has_line_numbers: false,
                    }],
                },
                status: ToolStatus::Created,
                content: relative_path,
            })
        }

        "bash" => {
            let command = extract_string_param(params, "command", "bash").unwrap_or_default();

            Some(ToolState {
                index: None,
                tool_name: "bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: command.to_string(),
                    result: None,
                },
                status: ToolStatus::Created,
                content: command.to_string(),
            })
        }

        // Return None for unknown/unsupported tools rather than failing
        _ => {
            tracing::debug!("Pi tool '{}' is not supported for log normalization", tool);
            None
        }
    }
}

fn update_tool_state_with_output(state: &mut ToolState, output: Value) {
    if state.tool_name == "bash" {
        if let ActionType::CommandRun { command: _, result } = &mut state.action_type {
            let (output_str, exit_code) = if let Some(obj) = output.as_object() {
                // Check if Pi's RPC result includes exit code information
                let code = obj
                    .get("exitCode")
                    .or_else(|| obj.get("exit_code"))
                    .or_else(|| obj.get("code"))
                    .and_then(|v| v.as_i64())
                    .map(|c| c as i32);

                let output_text = obj
                    .get("output")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        obj.get("stdout")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| serde_json::to_string_pretty(&output).unwrap_or_default());

                (output_text, code)
            } else if let Some(s) = output.as_str() {
                (s.to_string(), None)
            } else {
                (
                    serde_json::to_string_pretty(&output).unwrap_or_default(),
                    None,
                )
            };

            *result = Some(CommandRunResult {
                exit_status: exit_code.map(|code| CommandExitStatus::ExitCode { code }),
                output: Some(output_str),
            });
        }
    }
}

#[derive(Debug, Clone)]
struct ToolState {
    index: Option<usize>,
    tool_name: String,
    action_type: ActionType,
    status: ToolStatus,
    content: String,
}

impl ToolState {
    fn to_normalized_entry(&self) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: self.tool_name.clone(),
                action_type: self.action_type.clone(),
                status: self.status.clone(),
            },
            content: self.content.clone(),
            metadata: None,
        }
    }
}

/// Pi RPC Event types
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PiEvent {
    AgentStart {
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        model: Option<String>,
    },
    AgentEnd {
        messages: Vec<Value>,
    },
    MessageStart {
        message: Value,
    },
    MessageEnd {
        message: Value,
    },
    MessageUpdate {
        message: Value,
        #[serde(rename = "assistantMessageEvent")]
        assistant_message_event: AssistantMessageEvent,
    },
    TurnStart,
    TurnEnd {
        message: Value,
        #[serde(default, rename = "toolResults")]
        tool_results: Option<Vec<Value>>,
    },
    ToolExecutionStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        args: Value,
    },
    ToolExecutionUpdate {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        args: Value,
        #[serde(default, rename = "partialResult")]
        partial_result: Option<Value>,
    },
    ToolExecutionEnd {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        result: Value,
        #[serde(default, rename = "isError")]
        is_error: Option<bool>,
    },
    Error {
        error: String,
    },
    Response {
        command: String,
        #[serde(default)]
        success: bool,
        #[serde(default)]
        data: Option<Value>,
        #[serde(default)]
        error: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AssistantMessageEvent {
    ThinkingStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
    },
    ThinkingDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
    },
    ThinkingEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: String,
    },
    TextStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
    },
    TextDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
    },
    TextEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: String,
    },
    ToolcallStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
    },
    ToolcallDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
    },
    ToolcallEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        #[serde(rename = "toolCall")]
        tool_call: Value,
    },
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use uuid::Uuid;
    use workspace_utils::{log_msg::LogMsg, msg_store::MsgStore};

    use super::*;
    use crate::logs::{
        NormalizedEntry, NormalizedEntryType,
        utils::{EntryIndexProvider, patch::extract_normalized_entry_from_patch},
    };

    fn find_assistant_message(history: &[LogMsg]) -> Option<NormalizedEntry> {
        history.iter().find_map(|msg| {
            if let LogMsg::JsonPatch(patch) = msg {
                extract_normalized_entry_from_patch(patch).and_then(|(_, entry)| {
                    matches!(entry.entry_type, NormalizedEntryType::AssistantMessage)
                        .then_some(entry)
                })
            } else {
                None
            }
        })
    }

    fn find_entries_by_type(
        history: &[LogMsg],
        match_fn: impl Fn(&NormalizedEntryType) -> bool,
    ) -> Vec<NormalizedEntry> {
        history
            .iter()
            .filter_map(|msg| {
                if let LogMsg::JsonPatch(patch) = msg {
                    extract_normalized_entry_from_patch(patch)
                        .and_then(|(_, entry)| match_fn(&entry.entry_type).then_some(entry))
                } else {
                    None
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn test_session_discovery_does_not_block_log_loop() {
        let msg_store = Arc::new(MsgStore::new());
        let entry_index_provider = EntryIndexProvider::test_new();
        let worktree_path =
            std::env::temp_dir().join(format!("pi-session-test-{}", Uuid::new_v4()));

        normalize_logs(msg_store.clone(), &worktree_path, entry_index_provider);

        msg_store.push_stdout("{\"type\":\"agent_start\"}\n".to_string());
        msg_store.push_stdout(
            "{\"type\":\"message_update\",\"message\":{},\"assistantMessageEvent\":{\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"Hello\"}}\n"
                .to_string(),
        );
        msg_store.push_stdout("{\"type\":\"turn_end\",\"message\":{}}\n".to_string());
        msg_store.push_finished();

        let wait_for_message = async {
            loop {
                if let Some(entry) = find_assistant_message(&msg_store.get_history()) {
                    return entry;
                }
                tokio::task::yield_now().await;
            }
        };

        let entry = tokio::time::timeout(Duration::from_millis(250), wait_for_message)
            .await
            .expect("assistant message normalization timed out");

        assert!(entry.content.contains("Hello"));
    }

    #[tokio::test]
    async fn test_thinking_content_is_normalized() {
        let msg_store = Arc::new(MsgStore::new());
        let entry_index_provider = EntryIndexProvider::test_new();
        let worktree_path =
            std::env::temp_dir().join(format!("pi-thinking-test-{}", Uuid::new_v4()));

        normalize_logs(msg_store.clone(), &worktree_path, entry_index_provider);

        msg_store.push_stdout("{\"type\":\"agent_start\"}\n".to_string());
        msg_store.push_stdout(
            "{\"type\":\"message_update\",\"message\":{},\"assistantMessageEvent\":{\"type\":\"thinking_delta\",\"contentIndex\":0,\"delta\":\"Let me \"}}\n".to_string(),
        );
        msg_store.push_stdout(
            "{\"type\":\"message_update\",\"message\":{},\"assistantMessageEvent\":{\"type\":\"thinking_end\",\"contentIndex\":0,\"content\":\"Let me think about this\"}}\n".to_string(),
        );
        msg_store.push_stdout(
            "{\"type\":\"message_update\",\"message\":{},\"assistantMessageEvent\":{\"type\":\"text_delta\",\"contentIndex\":1,\"delta\":\"Here is my answer\"}}\n".to_string(),
        );
        msg_store.push_stdout("{\"type\":\"turn_end\",\"message\":{}}\n".to_string());
        msg_store.push_finished();

        let wait_for_entries = async {
            loop {
                let history = msg_store.get_history();
                let thinking = find_entries_by_type(&history, |t| {
                    matches!(t, NormalizedEntryType::Thinking)
                });
                let assistant = find_entries_by_type(&history, |t| {
                    matches!(t, NormalizedEntryType::AssistantMessage)
                });
                if !thinking.is_empty() && !assistant.is_empty() {
                    return (thinking, assistant);
                }
                tokio::task::yield_now().await;
            }
        };

        let (thinking, assistant) =
            tokio::time::timeout(Duration::from_millis(250), wait_for_entries)
                .await
                .expect("thinking + assistant normalization timed out");

        assert_eq!(thinking.last().unwrap().content, "Let me think about this");
        assert!(assistant[0].content.contains("Here is my answer"));
    }

    #[tokio::test]
    async fn test_rpc_response_error_is_surfaced() {
        let msg_store = Arc::new(MsgStore::new());
        let entry_index_provider = EntryIndexProvider::test_new();
        let worktree_path =
            std::env::temp_dir().join(format!("pi-rpc-error-test-{}", Uuid::new_v4()));

        normalize_logs(msg_store.clone(), &worktree_path, entry_index_provider);

        msg_store.push_stdout(
            "{\"type\":\"response\",\"command\":\"prompt\",\"success\":false,\"error\":\"invalid prompt format\"}\n".to_string(),
        );
        msg_store.push_finished();

        let wait_for_error = async {
            loop {
                let errors = find_entries_by_type(&msg_store.get_history(), |t| {
                    matches!(t, NormalizedEntryType::ErrorMessage { .. })
                });
                if !errors.is_empty() {
                    return errors;
                }
                tokio::task::yield_now().await;
            }
        };

        let errors = tokio::time::timeout(Duration::from_millis(250), wait_for_error)
            .await
            .expect("RPC error normalization timed out");

        assert!(errors[0].content.contains("invalid prompt format"));
    }

    #[tokio::test]
    async fn test_message_content_flushed_on_stream_end() {
        let msg_store = Arc::new(MsgStore::new());
        let entry_index_provider = EntryIndexProvider::test_new();
        let worktree_path =
            std::env::temp_dir().join(format!("pi-flush-test-{}", Uuid::new_v4()));

        normalize_logs(msg_store.clone(), &worktree_path, entry_index_provider);

        // Send text deltas but no TurnEnd/AgentEnd — simulate a crash
        msg_store.push_stdout("{\"type\":\"agent_start\"}\n".to_string());
        msg_store.push_stdout(
            "{\"type\":\"message_update\",\"message\":{},\"assistantMessageEvent\":{\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"partial content\"}}\n".to_string(),
        );
        msg_store.push_finished();

        let wait_for_message = async {
            loop {
                if let Some(entry) = find_assistant_message(&msg_store.get_history()) {
                    return entry;
                }
                tokio::task::yield_now().await;
            }
        };

        let entry = tokio::time::timeout(Duration::from_millis(250), wait_for_message)
            .await
            .expect("stream-end flush timed out");

        assert_eq!(entry.content, "partial content");
    }

    #[test]
    fn pi_edit_diff_uses_relative_path_snapshot() {
        let params = json!({
            "path": "/worktree/src/example.txt",
            "oldText": "old\n",
            "newText": "new\n"
        });

        let state =
            create_tool_state("edit", &params, "/worktree").expect("edit tool state");

        let diff = match &state.action_type {
            ActionType::FileEdit { changes, .. } => match &changes[0] {
                FileChange::Edit { unified_diff, .. } => unified_diff.clone(),
                _ => panic!("expected edit diff change"),
            },
            _ => panic!("expected file edit action type"),
        };

        assert_eq!(
            diff,
            "--- a/src/example.txt\n+++ b/src/example.txt\n@@ -1 +1 @@\n-old\n+new\n"
        );
    }
}
