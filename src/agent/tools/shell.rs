//! Shell execution tools - Full bash/cmd/powershell execution
//!
//! Provides unrestricted shell access (with permission system).

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// BashTool - Full shell execution (like Claude Code's bash tool)
// ============================================================================

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command with full access. Use for: running build commands, installing packages, git operations, running scripts, system commands. REQUIRES APPROVAL. On Windows uses PowerShell, on Unix uses bash."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for the command (optional)"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120)",
                    "default": 120
                },
                "stdin": {
                    "type": "string",
                    "description": "Optional input to send to stdin"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let command_str = params["command"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("command is required".into()))?;
        let working_dir = params["working_dir"].as_str();
        let timeout_secs = params["timeout_secs"].as_u64().unwrap_or(120);
        let stdin_input = params["stdin"].as_str();

        // Build command
        let (shell, shell_arg) = if cfg!(windows) {
            ("powershell", vec!["-NoProfile", "-Command"])
        } else {
            ("bash", vec!["-c"])
        };

        let mut cmd = Command::new(shell);
        for arg in &shell_arg {
            cmd.arg(arg);
        }
        cmd.arg(command_str);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Handle stdin
        if stdin_input.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Execute with timeout
        let result = timeout(Duration::from_secs(timeout_secs), async {
            let mut child = cmd.spawn().map_err(|e| {
                ToolError::ExecutionFailed(format!("Impossible de lancer la commande: {}", e))
            })?;

            if let Some(input) = stdin_input {
                if let Some(mut stdin) = child.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin.write_all(input.as_bytes()).await;
                    drop(stdin);
                }
            }

            child
                .wait_with_output()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Erreur d'exécution: {}", e)))
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                // Truncate very long output
                let stdout_display = truncate_output(&stdout, 50000);
                let stderr_display = truncate_output(&stderr, 10000);

                Ok(ToolResult {
                    success: output.status.success(),
                    data: serde_json::json!({
                        "stdout": stdout_display,
                        "stderr": stderr_display,
                        "exit_code": exit_code,
                        "command": command_str
                    }),
                    message: if output.status.success() {
                        format!("Commande exécutée (code: {})", exit_code)
                    } else {
                        format!("Commande échouée (code: {})", exit_code)
                    },
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ToolError::Timeout),
        }
    }
}

// ============================================================================
// BashBackgroundTool - Run commands in background
// ============================================================================

pub struct BashBackgroundTool;

#[async_trait]
impl Tool for BashBackgroundTool {
    fn name(&self) -> &str {
        "bash_background"
    }

    fn description(&self) -> &str {
        "Start a long-running shell command in the background (e.g., dev servers, watchers). Returns immediately with a process ID. REQUIRES APPROVAL."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to run in background"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory (optional)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let command_str = params["command"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("command is required".into()))?;
        let working_dir = params["working_dir"].as_str();

        let (shell, shell_arg) = if cfg!(windows) {
            ("powershell", vec!["-NoProfile", "-Command"])
        } else {
            ("bash", vec!["-c"])
        };

        let mut cmd = Command::new(shell);
        for arg in &shell_arg {
            cmd.arg(arg);
        }
        cmd.arg(command_str);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().map_err(|e| {
            ToolError::ExecutionFailed(format!("Impossible de lancer la commande: {}", e))
        })?;

        let pid = child.id().unwrap_or(0);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "pid": pid,
                "command": command_str,
                "status": "running"
            }),
            message: format!("Commande lancée en arrière-plan (PID: {})", pid),
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        output.to_string()
    } else {
        let half = max_chars / 2;
        let start = &output[..half];
        let end = &output[output.len() - half..];
        format!(
            "{}\n\n... [tronqué, {} caractères omis] ...\n\n{}",
            start,
            output.len() - max_chars,
            end
        )
    }
}
