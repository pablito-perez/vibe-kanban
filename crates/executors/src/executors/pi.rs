use std::{path::Path, process::Stdio, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncCommandGroup;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    sync::watch,
    time::{Duration, sleep},
};
use ts_rs::TS;
use workspace_utils::{msg_store::MsgStore, stream_lines::LinesStreamExt};

use crate::{
    command::{CommandBuildError, CommandBuilder, CommandParts},
    env::ExecutionEnv,
    executors::{AppendPrompt, ExecutorError, SpawnedChild, StandardCodingAgentExecutor},
    logs::utils::EntryIndexProvider,
    stdout_dup::duplicate_stdout,
};

pub mod normalize_logs;
pub mod session;

use normalize_logs::{extract_session_id_from_state, normalize_logs};

use self::session::fork_session;

// Keep the retry cadence aligned with Pi's filesystem discovery (~5.4s total)
// so we don't add extra latency if RPC `get_state` is slow.
const GET_STATE_RETRY_DELAYS_MS: [u64; 6] = [0, 300, 600, 1000, 1500, 2000];
const PI_NPM_PACKAGE: &str = "@mariozechner/pi-coding-agent";
const PI_NPM_PACKAGE_VERSION: &str = "0.52.9";

async fn write_rpc_message(
    stdin: &mut tokio::process::ChildStdin,
    payload: &serde_json::Value,
) -> Result<(), ExecutorError> {
    let message_str = serde_json::to_string(payload).map_err(ExecutorError::Json)?;
    stdin.write_all(message_str.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    Ok(())
}

fn try_extract_session_id_from_get_state_line(line: &str) -> Option<String> {
    // We parse the raw stdout line as JSON to find `get_state` responses.
    // This is intentionally local to Pi to avoid modifying the global executor pipeline.
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "response" {
        return None;
    }
    if value.get("command")?.as_str()? != "get_state" {
        return None;
    }
    if !value.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }

    let data = value.get("data").cloned();
    extract_session_id_from_state(&data)
}

/// Pi executor configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct Pi {
    #[serde(default)]
    pub append_prompt: AppendPrompt,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Model",
        description = "Model to use (e.g., claude-sonnet-4-5-20250929, gpt-4o, mistral-large-latest)"
    )]
    pub model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Provider",
        description = "LLM provider (e.g., anthropic, openai, mistral)"
    )]
    pub provider: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Auto Compaction",
        description = "Automatically compact context when approaching limits"
    )]
    pub auto_compaction: Option<bool>,

    #[serde(flatten)]
    pub cmd: crate::command::CmdOverrides,
}

impl Pi {
    pub fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        use crate::command::{CommandBuilder, apply_overrides};

        let mut builder = CommandBuilder::new(format!(
            "npx -y {PI_NPM_PACKAGE}@{PI_NPM_PACKAGE_VERSION}"
        ))
        .params(["--mode", "rpc"]);

        // Set model if specified
        if let Some(model) = &self.model {
            builder = builder.extend_params(["--model", model.as_str()]);
        }

        // Set provider if specified
        if let Some(provider) = &self.provider {
            builder = builder.extend_params(["--provider", provider.as_str()]);
        }

        // Set auto-compaction (default to true if not specified)
        if self.auto_compaction.unwrap_or(true) {
            builder = builder.extend_params(["--auto-compaction"]);
        }

        apply_overrides(builder, &self.cmd)
    }
}

async fn spawn_pi(
    command_parts: CommandParts,
    prompt: &str,
    current_dir: &Path,
    env: &ExecutionEnv,
    cmd_overrides: &crate::command::CmdOverrides,
) -> Result<SpawnedChild, ExecutorError> {
    let (program_path, args) = command_parts.into_resolved().await?;

    let mut command = Command::new(program_path);
    command
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(current_dir)
        .env("NPM_CONFIG_LOGLEVEL", "error")
        .args(args);

    env.clone()
        .with_profile(cmd_overrides)
        .apply_to_command(&mut command);

    let mut child = command.group_spawn()?;

    // Duplicate stdout so we can observe `get_state` responses without
    // stealing the stream from the container log pipeline.
    // Trade-off: if we cannot duplicate stdout, we fail spawn rather than
    // proceeding with ambiguous logging behavior.
    let stdout_dup = duplicate_stdout(&mut child)?;
    let mut stdout_lines = stdout_dup.lines();

    // Shared signal to stop stdin polling once we observe a session_id.
    let (session_ready_tx, mut session_ready_rx) = watch::channel(false);
    tokio::spawn(async move {
        while let Some(Ok(line)) = stdout_lines.next().await {
            if try_extract_session_id_from_get_state_line(&line).is_some() {
                let _ = session_ready_tx.send(true);
                break;
            }
        }
    });

    // Send RPC prompt message via stdin, then poll get_state until ready.
    if let Some(mut stdin) = child.inner().stdin.take() {
        let rpc_message = serde_json::json!({
            "type": "prompt",
            "message": prompt
        });

        write_rpc_message(&mut stdin, &rpc_message).await?;

        tokio::spawn(async move {
            for delay_ms in GET_STATE_RETRY_DELAYS_MS {
                if delay_ms > 0 {
                    tokio::select! {
                        _ = sleep(Duration::from_millis(delay_ms)) => {},
                        _ = session_ready_rx.changed() => {},
                    }
                }

                if *session_ready_rx.borrow() {
                    break;
                }

                let get_state_message = serde_json::json!({
                    "type": "get_state"
                });

                if let Err(err) = write_rpc_message(&mut stdin, &get_state_message).await {
                    tracing::debug!("Failed to send Pi get_state command: {}", err);
                    break;
                }
            }

            // Close stdin once we either observe a session_id or exhaust retries.
            // This avoids a long-lived stdin pipe while still allowing an early stop.
            let _ = stdin.shutdown().await;
        });
    }

    Ok(child.into())
}

#[async_trait]
impl StandardCodingAgentExecutor for Pi {
    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let pi_command = self.build_command_builder()?.build_initial()?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);

        spawn_pi(pi_command, &combined_prompt, current_dir, env, &self.cmd).await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        _reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // Fork the session first (if Pi supports session forking)
        let forked_session_path = fork_session(session_id).map_err(|e| {
            ExecutorError::FollowUpNotSupported(format!(
                "Failed to fork Pi session {session_id}: {e}"
            ))
        })?;

        let session_path_str = forked_session_path.to_string_lossy().to_string();
        let continue_cmd = self
            .build_command_builder()?
            .build_follow_up(&["--session".to_string(), session_path_str])?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);

        spawn_pi(continue_cmd, &combined_prompt, current_dir, env, &self.cmd).await
    }

    fn normalize_logs(&self, msg_store: Arc<MsgStore>, current_dir: &Path) {
        normalize_logs(
            msg_store.clone(),
            current_dir,
            EntryIndexProvider::start_from(&msg_store),
        );
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        // Pi uses extensions, not MCP servers
        // We check for Pi's config directory to determine if it's installed
        dirs::home_dir().map(|home| home.join(".pi").join("agent").join("config.toml"))
    }

    fn get_availability_info(&self) -> crate::executors::AvailabilityInfo {
        // Check if Pi is installed by looking for common Pi directories
        let pi_config_found = dirs::home_dir()
            .map(|home| {
                let config_path = home.join(".pi").join("agent").join("config.toml");
                let sessions_path = home.join(".pi").join("agent").join("sessions");
                config_path.exists() || sessions_path.exists()
            })
            .unwrap_or(false);

        if pi_config_found {
            crate::executors::AvailabilityInfo::InstallationFound
        } else {
            crate::executors::AvailabilityInfo::NotFound
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::command::CmdOverrides;

    use super::*;

    #[test]
    fn pi_follow_up_args_extend_initial() {
        let pi = Pi {
            append_prompt: AppendPrompt::default(),
            model: Some("test-model".to_string()),
            provider: Some("test-provider".to_string()),
            auto_compaction: Some(false),
            cmd: CmdOverrides::default(),
        };

        let builder = pi.build_command_builder().unwrap();
        let (initial_program, initial_args) = builder.build_initial().unwrap().into_parts();
        let (follow_up_program, follow_up_args) = builder
            .build_follow_up(&["--session".to_string(), "session-path".to_string()])
            .unwrap()
            .into_parts();

        assert_eq!(initial_program, follow_up_program);
        assert!(follow_up_args.starts_with(&initial_args));
        assert!(follow_up_args.ends_with(&["--session".to_string(), "session-path".to_string()]));
    }
}
