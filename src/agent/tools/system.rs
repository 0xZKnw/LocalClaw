//! System tools - Process list, Environment, System info
//!
//! Provides system-level information and operations.

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// ProcessListTool - List running processes
// ============================================================================

pub struct ProcessListTool;

#[async_trait]
impl Tool for ProcessListTool {
    fn name(&self) -> &str {
        "process_list"
    }

    fn description(&self) -> &str {
        "List running processes on the system. Can filter by name."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "Filter processes by name (optional)"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let filter = params["filter"].as_str();

        let output = if cfg!(windows) {
            Command::new("powershell")
                .args(["-NoProfile", "-Command", "Get-Process | Select-Object Id, ProcessName, CPU, WorkingSet64 | Format-Table -AutoSize"])
                .output()
                .await
        } else {
            Command::new("ps")
                .args(["aux", "--sort=-%cpu"])
                .output()
                .await
        };

        let output = output.map_err(|e| {
            ToolError::ExecutionFailed(format!("Impossible de lister les processus: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        let filtered = if let Some(f) = filter {
            let f_lower = f.to_lowercase();
            stdout
                .lines()
                .filter(|l| l.to_lowercase().contains(&f_lower) || l.contains("PID") || l.contains("Id"))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // Limit to first 50 lines
            stdout.lines().take(50).collect::<Vec<_>>().join("\n")
        };

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "processes": filtered,
                "filter": filter
            }),
            message: format!("Processus listés{}", 
                filter.map(|f| format!(" (filtre: {})", f)).unwrap_or_default()),
        })
    }
}

// ============================================================================
// EnvironmentTool - Get/set environment variables
// ============================================================================

pub struct EnvironmentTool;

#[async_trait]
impl Tool for EnvironmentTool {
    fn name(&self) -> &str {
        "environment"
    }

    fn description(&self) -> &str {
        "Get environment variables. Can list all or get a specific variable."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Specific environment variable name (omit to list all)"
                },
                "filter": {
                    "type": "string",
                    "description": "Filter variable names (case-insensitive)"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let name = params["name"].as_str();
        let filter = params["filter"].as_str();

        if let Some(var_name) = name {
            match std::env::var(var_name) {
                Ok(value) => Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({
                        "name": var_name,
                        "value": value
                    }),
                    message: format!("{}={}", var_name, value),
                }),
                Err(_) => Ok(ToolResult {
                    success: false,
                    data: serde_json::json!({ "name": var_name, "value": null }),
                    message: format!("Variable '{}' non définie", var_name),
                }),
            }
        } else {
            let mut vars: Vec<(String, String)> = std::env::vars().collect();
            vars.sort_by(|a, b| a.0.cmp(&b.0));

            if let Some(f) = filter {
                let f_lower = f.to_lowercase();
                vars.retain(|(k, _)| k.to_lowercase().contains(&f_lower));
            }

            // Don't expose sensitive variables
            let sensitive = ["PASSWORD", "SECRET", "TOKEN", "KEY", "CREDENTIALS", "API_KEY"];
            let safe_vars: Vec<Value> = vars
                .iter()
                .map(|(k, v)| {
                    let is_sensitive = sensitive.iter().any(|s| k.to_uppercase().contains(s));
                    serde_json::json!({
                        "name": k,
                        "value": if is_sensitive { "***MASKED***".to_string() } else { v.clone() }
                    })
                })
                .collect();

            Ok(ToolResult {
                success: true,
                data: serde_json::json!({
                    "variables": safe_vars,
                    "count": safe_vars.len()
                }),
                message: format!("{} variable(s) d'environnement", safe_vars.len()),
            })
        }
    }
}

// ============================================================================
// SystemInfoTool - Get system information
// ============================================================================

pub struct SystemInfoTool;

#[async_trait]
impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }

    fn description(&self) -> &str {
        "Get system information: OS, architecture, hostname, working directory, disk space."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _params: Value) -> Result<ToolResult, ToolError> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let home = dirs_home();

        // Get hostname
        let hostname = if cfg!(windows) {
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string())
        } else {
            std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("HOST"))
                .unwrap_or_else(|_| "unknown".to_string())
        };

        // Get username
        let username = if cfg!(windows) {
            std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
        } else {
            std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
        };

        // Get available disk space (best effort)
        let disk_info = get_disk_info().await;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "os": os,
                "arch": arch,
                "hostname": hostname,
                "username": username,
                "current_directory": cwd,
                "home_directory": home,
                "disk": disk_info,
                "num_cpus": std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1),
            }),
            message: format!("{} {} | {} | cwd: {}", os, arch, hostname, cwd),
        })
    }
}

// ============================================================================
// WhichTool - Find executable path
// ============================================================================

pub struct WhichTool;

#[async_trait]
impl Tool for WhichTool {
    fn name(&self) -> &str {
        "which"
    }

    fn description(&self) -> &str {
        "Find the full path of an executable command (like 'which' on Unix or 'where' on Windows)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command name to find"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let command_name = params["command"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("command is required".into()))?;

        let (cmd, args) = if cfg!(windows) {
            ("where", vec![command_name])
        } else {
            ("which", vec![command_name])
        };

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if output.status.success() && !stdout.is_empty() {
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({
                    "command": command_name,
                    "path": stdout
                }),
                message: format!("{}: {}", command_name, stdout),
            })
        } else {
            Ok(ToolResult {
                success: false,
                data: serde_json::json!({
                    "command": command_name,
                    "path": null
                }),
                message: format!("'{}' non trouvé dans le PATH", command_name),
            })
        }
    }
}

// ============================================================================
// TreeTool - Show directory tree
// ============================================================================

pub struct TreeTool;

#[async_trait]
impl Tool for TreeTool {
    fn name(&self) -> &str {
        "tree"
    }

    fn description(&self) -> &str {
        "Show directory structure as a tree. Useful for understanding project layout."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Root directory to display (default: current dir)",
                    "default": "."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth to display (default: 3)",
                    "default": 3
                },
                "show_hidden": {
                    "type": "boolean",
                    "description": "Show hidden files/directories",
                    "default": false
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let path = params["path"].as_str().unwrap_or(".");
        let max_depth = params["max_depth"].as_u64().unwrap_or(3) as usize;
        let show_hidden = params["show_hidden"].as_bool().unwrap_or(false);

        let path_buf = std::path::PathBuf::from(path);
        if !path_buf.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "Le chemin '{}' n'existe pas",
                path
            )));
        }

        let mut tree = String::new();
        let mut file_count = 0usize;
        let mut dir_count = 0usize;

        tree.push_str(&format!("{}\n", path));
        build_tree(
            &path_buf,
            "",
            max_depth,
            0,
            show_hidden,
            &mut tree,
            &mut file_count,
            &mut dir_count,
        )
        .await?;

        tree.push_str(&format!(
            "\n{} dossier(s), {} fichier(s)",
            dir_count, file_count
        ));

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "tree": tree,
                "files": file_count,
                "directories": dir_count
            }),
            message: format!(
                "Arborescence: {} dossier(s), {} fichier(s)",
                dir_count, file_count
            ),
        })
    }
}

fn build_tree<'a>(
    path: &'a std::path::PathBuf,
    prefix: &'a str,
    max_depth: usize,
    depth: usize,
    show_hidden: bool,
    tree: &'a mut String,
    file_count: &'a mut usize,
    dir_count: &'a mut usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ToolError>> + Send + 'a>> {
    Box::pin(async move {
        if depth >= max_depth {
            return Ok(());
        }

        let mut entries: Vec<_> = Vec::new();
        let mut read_dir = match tokio::fs::read_dir(path).await {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            if name == "node_modules" || name == "target" || name == "__pycache__" || name == ".git"
            {
                continue;
            }
            entries.push(entry);
        }

        entries.sort_by(|a, b| {
            let a_name = a.file_name().to_string_lossy().to_string();
            let b_name = b.file_name().to_string_lossy().to_string();
            a_name.cmp(&b_name)
        });

        let count = entries.len();
        for (i, entry) in entries.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);

            tree.push_str(&format!("{}{}{}", prefix, connector, name));
            if is_dir {
                tree.push_str("/\n");
                *dir_count += 1;
                let new_prefix = format!("{}{}",
                    prefix,
                    if is_last { "    " } else { "│   " }
                );
                build_tree(
                    &entry.path(),
                    &new_prefix,
                    max_depth,
                    depth + 1,
                    show_hidden,
                    tree,
                    file_count,
                    dir_count,
                )
                .await?;
            } else {
                tree.push('\n');
                *file_count += 1;
            }
        }

        Ok(())
    })
}

// ============================================================================
// Helpers
// ============================================================================

fn dirs_home() -> String {
    if cfg!(windows) {
        std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users".to_string())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/home".to_string())
    }
}

async fn get_disk_info() -> Value {
    let output = if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='UsedGB';E={[math]::Round($_.Used/1GB,2)}}, @{N='FreeGB';E={[math]::Round($_.Free/1GB,2)}} | ConvertTo-Json",
            ])
            .output()
            .await
    } else {
        Command::new("df")
            .args(["-h", "--output=target,size,used,avail,pcent"])
            .output()
            .await
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            serde_json::json!({ "info": stdout.trim() })
        }
        Err(_) => serde_json::json!({ "info": "unavailable" }),
    }
}
