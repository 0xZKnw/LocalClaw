use async_trait::async_trait;
use serde_json::Value;

use crate::agent::tools::{Tool, ToolError, ToolResult};
use crate::agent::tools::mcp_client::{McpServerConfig, McpTransport};
use crate::agent::mcp_config;

/// Tool to add an MCP server
pub struct McpAddServerTool;

#[async_trait]
impl Tool for McpAddServerTool {
    fn name(&self) -> &str {
        "mcp_add_server"
    }

    fn description(&self) -> &str {
        "Add a new MCP server configuration to the global mcp.json file."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Unique identifier for the server (e.g., 'filesystem', 'git')"
                },
                "name": {
                    "type": "string",
                    "description": "Human readable name"
                },
                "type": {
                    "type": "string",
                    "enum": ["stdio", "http"],
                    "description": "Transport type"
                },
                "command": {
                    "type": "string",
                    "description": "Command to execute (for stdio)"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Arguments for command (for stdio)"
                },
                "url": {
                    "type": "string",
                    "description": "URL to connect to (for http)"
                },
                "env": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Environment variables"
                }
            },
            "required": ["id", "name", "type"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let id = params["id"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("id is required".to_string()))?
            .to_string();
        
        let name = params["name"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("name is required".to_string()))?
            .to_string();
            
        let transport_type = params["type"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("type is required".to_string()))?;

        let transport = match transport_type {
            "stdio" => {
                let command = params["command"].as_str()
                    .ok_or_else(|| ToolError::InvalidParameters("command is required for stdio".to_string()))?
                    .to_string();
                    
                let args = params["args"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                    
                McpTransport::Stdio { command, args }
            },
            "http" => {
                let url = params["url"].as_str()
                    .ok_or_else(|| ToolError::InvalidParameters("url is required for http".to_string()))?
                    .to_string();
                    
                McpTransport::Http { url }
            },
            _ => return Err(ToolError::InvalidParameters("Invalid transport type".to_string())),
        };
        
        let env_map = params["env"].as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let new_config = McpServerConfig {
            id: id.clone(),
            name,
            transport,
            env: env_map,
            enabled: true,
        };
        
        mcp_config::add_server(new_config).await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to add server: {}", e)))?;
        
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({ "id": id }),
            message: format!("Serveur MCP '{}' ajouté. Redémarrez l'agent pour appliquer.", id),
        })
    }
}

/// Tool to list MCP servers
pub struct McpListServersTool;

#[async_trait]
impl Tool for McpListServersTool {
    fn name(&self) -> &str {
        "mcp_list_servers"
    }

    fn description(&self) -> &str {
        "List all effective MCP servers (merging presets and local config)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(&self, _params: Value) -> Result<ToolResult, ToolError> {
        let configs = mcp_config::load_effective_config().await;
        
        let values: Vec<Value> = configs.into_iter().map(|c| {
            serde_json::json!({
                "id": c.id,
                "name": c.name,
                "enabled": c.enabled,
                "config": c
            })
        }).collect();
        
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({ "servers": values }),
            message: format!("{} serveurs configurés", values.len()),
        })
    }
}

/// Tool to remove an MCP server
pub struct McpRemoveServerTool;

#[async_trait]
impl Tool for McpRemoveServerTool {
    fn name(&self) -> &str {
        "mcp_remove_server"
    }

    fn description(&self) -> &str {
        "Remove an MCP server from the local configuration."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "ID of the server to remove"
                }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let id = params["id"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("id is required".to_string()))?;
            
        mcp_config::remove_server(id).await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to remove server: {}", e)))?;
        
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({ "id": id }),
            message: format!("Serveur MCP '{}' supprimé.", id),
        })
    }
}
