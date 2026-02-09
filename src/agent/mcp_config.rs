//! MCP Configuration Manager
//!
//! Handles parsing and loading of MCP configurations from:
//! 1. Global mcp.json (~/.localm/mcp.json)
//! 2. Project-local mcp.json (./.localm/mcp.json)
//!
//! Note: Presets are available via get_all_presets() for UI suggestions,
//! but are NOT automatically loaded as active servers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::agent::tools::mcp_client::{McpServerConfig, McpTransport};
use crate::storage::get_data_dir;

/// JSON structure matching Claude Desktop's mcp.json format
#[derive(Serialize, Deserialize, Debug)]
struct McpJsonConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpJsonServerConfig>,
}

/// Server configuration within mcp.json
#[derive(Serialize, Deserialize, Debug)]
struct McpJsonServerConfig {
    /// Command to execute (for stdio)
    command: Option<String>,
    /// Arguments for the command (for stdio)
    args: Option<Vec<String>>,
    /// Environment variables
    env: Option<HashMap<String, String>>,
    /// URL (for http/sse)
    url: Option<String>,
}

/// Load MCP configurations from mcp.json files only
/// 
/// Priority (highest to lowest):
/// 1. Local project config (./.localm/mcp.json)
/// 2. Global config (~/.localm/mcp.json)
/// 
/// Note: Presets are NOT automatically loaded. Use get_available_presets() 
/// to show preset suggestions in UI.
pub async fn load_effective_config() -> Vec<McpServerConfig> {
    let mut config_map: HashMap<String, McpServerConfig> = HashMap::new();

    // 1. Load Global Config
    if let Ok(data_dir) = get_data_dir() {
        let global_config_path = data_dir.join("mcp.json");
        if let Some(configs) = load_config_from_file(&global_config_path).await {
            tracing::info!("Loaded global MCP config from {}", global_config_path.display());
            merge_configs(&mut config_map, configs);
        }
    }

    // 2. Load Local Config (current directory)
    // We look for .localm/mcp.json in the current working directory
    let local_config_path = PathBuf::from(".localm").join("mcp.json");
    if let Some(configs) = load_config_from_file(&local_config_path).await {
        tracing::info!("Loaded local MCP config from {}", local_config_path.display());
        merge_configs(&mut config_map, configs);
    }

    // Convert map back to vector
    config_map.into_values().collect()
}

/// Get available preset configurations (for UI suggestions)
pub fn get_available_presets() -> Vec<McpServerConfig> {
    crate::agent::tools::mcp_presets::get_all_presets()
        .into_iter()
        .map(|p| p.config)
        .collect()
}

/// Merge new configs into the map, overriding existing ones with same ID
fn merge_configs(
    base: &mut HashMap<String, McpServerConfig>, 
    new_configs: Vec<McpServerConfig>
) {
    for config in new_configs {
        // If it exists, we overwrite it (user config overrides preset)
        // Note: This effectively "enables" a preset if the user defines it in JSON,
        // because our parse logic defaults enabled=true
        base.insert(config.id.clone(), config);
    }
}

/// Add or update a server in the global config (mcp.json)
pub async fn add_server(config: McpServerConfig) -> Result<(), String> {
    let data_dir = get_data_dir().map_err(|e| e.to_string())?;
    let config_path = data_dir.join("mcp.json");
    
    // Read existing
    let mut json_config = if config_path.exists() {
        let content = fs::read_to_string(&config_path).await.map_err(|e| e.to_string())?;
        serde_json::from_str::<McpJsonConfig>(&content).map_err(|e| e.to_string())?
    } else {
        McpJsonConfig { mcp_servers: HashMap::new() }
    };
    
    // Convert McpServerConfig to McpJsonServerConfig
    let (cmd, args, url) = match config.transport {
        McpTransport::Stdio { command, args } => (Some(command), Some(args), None),
        McpTransport::Http { url } => (None, None, Some(url)),
    };
    
    let server_config = McpJsonServerConfig {
        command: cmd,
        args: args,
        env: Some(config.env),
        url: url,
    };
    
    json_config.mcp_servers.insert(config.id, server_config);
    
    // Write back
    let new_content = serde_json::to_string_pretty(&json_config).map_err(|e| e.to_string())?;
    fs::write(&config_path, new_content).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Remove a server from the global config
pub async fn remove_server(id: &str) -> Result<(), String> {
    let data_dir = get_data_dir().map_err(|e| e.to_string())?;
    let config_path = data_dir.join("mcp.json");
    
    if !config_path.exists() {
        return Err("Configuration file not found".to_string());
    }
    
    let content = fs::read_to_string(&config_path).await.map_err(|e| e.to_string())?;
    let mut json_config: McpJsonConfig = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    
    if json_config.mcp_servers.remove(id).is_none() {
        return Err(format!("Server '{}' not found in configuration", id));
    }
    
    let new_content = serde_json::to_string_pretty(&json_config).map_err(|e| e.to_string())?;
    fs::write(&config_path, new_content).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Parse a file into a list of McpServerConfig
async fn load_config_from_file(path: &Path) -> Option<Vec<McpServerConfig>> {
    if !path.exists() {
        return None;
    }

    let content = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read MCP config file {}: {}", path.display(), e);
            return None;
        }
    };

    let json_config: McpJsonConfig = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to parse MCP config file {}: {}", path.display(), e);
            return None;
        }
    };

    let mut configs = Vec::new();

    for (id, server_conf) in json_config.mcp_servers {
        // Determine transport
        let transport = if let Some(url) = server_conf.url {
            McpTransport::Http { url }
        } else if let Some(cmd) = server_conf.command {
            McpTransport::Stdio {
                command: cmd,
                args: server_conf.args.unwrap_or_default(),
            }
        } else {
            tracing::warn!("MCP server '{}' has neither 'url' nor 'command', skipping", id);
            continue;
        };

        // Create config object
        // We capitalize the ID to make a friendly name if one isn't provided (mcp.json format doesn't have name)
        let name = capitalize(&id);

        configs.push(McpServerConfig {
            id,
            name,
            transport,
            env: server_conf.env.unwrap_or_default(),
            enabled: true, // User-defined configs are enabled by default
        });
    }

    Some(configs)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
