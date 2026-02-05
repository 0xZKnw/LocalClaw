use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use dashmap::DashMap;
use thiserror::Error;

/// Tool trait - all tools must implement this
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError>;
}

/// Tool execution result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub data: Value,
    pub message: String,
}

/// Tool errors
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Timeout")]
    Timeout,
}

/// Tool information for listing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters_schema: Value,
}

/// Tool registry - singleton pattern
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
        }
    }
    
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).map(|t| t.clone())
    }
    
    pub fn list_tools(&self) -> Vec<ToolInfo> {
        self.tools
            .iter()
            .map(|entry| ToolInfo {
                name: entry.name().to_string(),
                description: entry.description().to_string(),
                parameters_schema: entry.parameters_schema(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Builtin tools module
pub mod builtins {
    use super::*;
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};
    use std::path::PathBuf;
    
    /// File read tool
    pub struct FileReadTool;
    
    #[async_trait]
    impl Tool for FileReadTool {
        fn name(&self) -> &str {
            "file_read"
        }
        
        fn description(&self) -> &str {
            "Read the contents of a file"
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to read"
                    }
                },
                "required": ["path"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let path = params["path"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("path is required".to_string()))?;
            
            let path = PathBuf::from(path);
            
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({ "content": content }),
                    message: format!("Successfully read file: {}", path.display()),
                }),
                Err(e) => Err(ToolError::ExecutionFailed(format!("Failed to read file: {}", e))),
            }
        }
    }
    
    /// File list tool
    pub struct FileListTool;
    
    #[async_trait]
    impl Tool for FileListTool {
        fn name(&self) -> &str {
            "file_list"
        }
        
        fn description(&self) -> &str {
            "List files in a directory"
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the directory"
                    }
                },
                "required": ["path"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let path = params["path"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("path is required".to_string()))?;
            
            let path = PathBuf::from(path);
            
            match tokio::fs::read_dir(&path).await {
                Ok(mut entries) => {
                    let mut files = Vec::new();
                    while let Some(entry) = entries.next_entry().await.map_err(|e| ToolError::ExecutionFailed(e.to_string()))? {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.file_type().await.map(|ft| ft.is_dir()).unwrap_or(false);
                        files.push(serde_json::json!({
                            "name": name,
                            "is_directory": is_dir,
                        }));
                    }
                    Ok(ToolResult {
                        success: true,
                        data: serde_json::json!({ "files": files }),
                        message: format!("Listed {} files in {}", files.len(), path.display()),
                    })
                }
                Err(e) => Err(ToolError::ExecutionFailed(format!("Failed to list directory: {}", e))),
            }
        }
    }
    
    /// Command execution tool
    pub struct CommandTool;
    
    #[async_trait]
    impl Tool for CommandTool {
        fn name(&self) -> &str {
            "command"
        }
        
        fn description(&self) -> &str {
            "Execute a shell command (requires approval)"
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30)",
                        "default": 30
                    }
                },
                "required": ["command"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let command_str = params["command"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("command is required".to_string()))?;
            
            let timeout_secs = params["timeout_secs"].as_u64().unwrap_or(30);
            
            // SECURITY: Only allow safe read-only commands
            let allowed_commands = ["ls", "cat", "echo", "pwd", "whoami", "date", "wc", "head", "tail", "find", "grep"];
            let cmd_parts: Vec<&str> = command_str.split_whitespace().collect();
            if cmd_parts.is_empty() {
                return Err(ToolError::InvalidParameters("Empty command".to_string()));
            }
            
            if !allowed_commands.contains(&cmd_parts[0]) {
                return Err(ToolError::PermissionDenied(
                    format!("Command '{}' is not in the allowed list", cmd_parts[0])
                ));
            }
            
            // Execute with timeout
            let result = timeout(
                Duration::from_secs(timeout_secs),
                Command::new("sh")
                    .arg("-c")
                    .arg(command_str)
                    .output()
            ).await;
            
            match result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    
                    Ok(ToolResult {
                        success: output.status.success(),
                        data: serde_json::json!({
                            "stdout": stdout,
                            "stderr": stderr,
                            "exit_code": output.status.code(),
                        }),
                        message: if output.status.success() {
                            "Command executed successfully".to_string()
                        } else {
                            format!("Command failed with exit code: {:?}", output.status.code())
                        },
                    })
                }
                Ok(Err(e)) => Err(ToolError::ExecutionFailed(format!("Failed to execute command: {}", e))),
                Err(_) => Err(ToolError::Timeout),
            }
        }
    }
}
