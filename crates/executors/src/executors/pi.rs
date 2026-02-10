use std::{path::Path, process::Stdio, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncCommandGroup;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, process::Command};
use ts_rs::TS;
use uuid::Uuid;
use workspace_utils::msg_store::MsgStore;

use crate::{
    command::{CommandBuildError, CommandBuilder, CommandParts},
    env::ExecutionEnv,
    executors::{AppendPrompt, ExecutorError, SpawnedChild, StandardCodingAgentExecutor},
    logs::utils::EntryIndexProvider,
};

pub mod normalize_logs;

use normalize_logs::normalize_logs;

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
    // Send the RPC prompt and close stdin.
    if let Some(mut stdin) = child.inner().stdin.take() {
        let rpc_message = serde_json::json!({
            "type": "prompt",
            "message": prompt
        });

        write_rpc_message(&mut stdin, &rpc_message).await?;
        stdin.shutdown().await?;
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
        // Generate a session_id upfront so that:
        // 1. Pi uses it immediately (no async discovery needed)
        // 2. normalize_logs picks it up from the agent_start event
        // 3. Follow-ups can reference it without race conditions
        let session_id = Uuid::new_v4().to_string();
        let pi_command = self
            .build_command_builder()?
            .build_follow_up(&["--session-id".to_string(), session_id])?;
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
        let continue_cmd = self
            .build_command_builder()?
            .build_follow_up(&["--session-id".to_string(), session_id.to_string()])?;
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
        // Pi uses its own extension system rather than MCP servers,
        // so we return None to signal that MCP is not supported.
        // Availability is handled separately in `get_availability_info`.
        None
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
    use super::*;
    use crate::command::CmdOverrides;

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
            .build_follow_up(&["--session-id".to_string(), "session-id".to_string()])
            .unwrap()
            .into_parts();

        assert_eq!(initial_program, follow_up_program);
        assert!(follow_up_args.starts_with(&initial_args));
        assert!(follow_up_args.ends_with(&["--session-id".to_string(), "session-id".to_string()]));
    }
}
