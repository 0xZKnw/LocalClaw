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

impl PartialEq for ToolResult {
    fn eq(&self, other: &Self) -> bool {
        self.success == other.success && self.message == other.message
    }
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
    
    pub fn register_sync(&self, tool: Arc<dyn Tool>) {
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
    
    pub fn count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Exa search tool
pub mod exa;

/// File system tools (edit, create, delete, move, info, mkdir, copy, search)
pub mod filesystem;

/// Shell execution tools (bash, background)
pub mod shell;

/// Git tools (status, diff, log, commit, branch, stash)
pub mod git;

/// Web tools (fetch, download)
pub mod web;

/// Developer tools (diff, find-replace, patch, wc)
pub mod dev;

/// System tools (process list, environment, system info, which, tree)
pub mod system;

/// PDF tools (read, create, add page, merge)
pub mod pdf;

/// Skill creation tool
pub mod skill_create;
pub mod skill_invoke;
pub mod skill_list;

/// Generic MCP client (stdio + HTTP transports)
pub mod mcp_client;

/// MCP server presets for popular services
pub mod mcp_presets;

/// MCP management tools
pub mod mcp_management;

/// Builtin tools module
pub mod builtins {
    use super::*;
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};
    use std::path::PathBuf;
    use glob::glob as glob_match;
    use regex::Regex;
    
    /// File read tool - improved with line numbers and range support
    pub struct FileReadTool;
    
    #[async_trait]
    impl Tool for FileReadTool {
        fn name(&self) -> &str {
            "file_read"
        }
        
        fn description(&self) -> &str {
            "Read the contents of a file. Can optionally read specific line ranges."
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file to read"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "Optional start line number (1-indexed)"
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Optional end line number (1-indexed)"
                    }
                },
                "required": ["path"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let path = params["path"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("path is required".to_string()))?;
            
            let path = PathBuf::from(path);
            let start_line = params["start_line"].as_u64().map(|n| n as usize);
            let end_line = params["end_line"].as_u64().map(|n| n as usize);
            
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let total_lines = lines.len();
                    
                    let (output, range_info) = match (start_line, end_line) {
                        (Some(start), Some(end)) => {
                            let start = start.saturating_sub(1).min(total_lines);
                            let end = end.min(total_lines);
                            let selected: Vec<String> = lines[start..end]
                                .iter()
                                .enumerate()
                                .map(|(i, l)| format!("{:>4}| {}", start + i + 1, l))
                                .collect();
                            (selected.join("\n"), format!(" (lignes {}-{})", start + 1, end))
                        }
                        (Some(start), None) => {
                            let start = start.saturating_sub(1).min(total_lines);
                            let selected: Vec<String> = lines[start..]
                                .iter()
                                .enumerate()
                                .map(|(i, l)| format!("{:>4}| {}", start + i + 1, l))
                                .collect();
                            (selected.join("\n"), format!(" (depuis ligne {})", start + 1))
                        }
                        _ => {
                            // Add line numbers for better context
                            let numbered: Vec<String> = lines
                                .iter()
                                .enumerate()
                                .map(|(i, l)| format!("{:>4}| {}", i + 1, l))
                                .collect();
                            (numbered.join("\n"), String::new())
                        }
                    };
                    
                    Ok(ToolResult {
                        success: true,
                        data: serde_json::json!({ 
                            "content": output,
                            "total_lines": total_lines,
                            "path": path.display().to_string()
                        }),
                        message: format!("Fichier lu: {} ({} lignes){}",
                            path.display(), total_lines, range_info),
                    })
                }
                Err(e) => Err(ToolError::ExecutionFailed(format!("Erreur lecture fichier: {}", e))),
            }
        }
    }
    
    /// File write tool
    pub struct FileWriteTool;
    
    #[async_trait]
    impl Tool for FileWriteTool {
        fn name(&self) -> &str {
            "file_write"
        }
        
        fn description(&self) -> &str {
            "Write content to a file. Creates the file if it doesn't exist. REQUIRES APPROVAL."
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "If true, append to file instead of overwriting",
                        "default": false
                    }
                },
                "required": ["path", "content"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let path = params["path"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("path is required".to_string()))?;
            let content = params["content"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("content is required".to_string()))?;
            let append = params["append"].as_bool().unwrap_or(false);
            
            let path = PathBuf::from(path);
            
            // Create parent directories if needed
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    tokio::fs::create_dir_all(parent).await
                        .map_err(|e| ToolError::ExecutionFailed(format!("Erreur création dossier: {}", e)))?;
                }
            }
            
            let result = if append {
                use tokio::io::AsyncWriteExt;
                let mut file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Erreur ouverture fichier: {}", e)))?;
                file.write_all(content.as_bytes()).await
            } else {
                tokio::fs::write(&path, content).await
            };
            
            match result {
                Ok(_) => {
                    let bytes = content.len();
                    let lines = content.lines().count();
                    Ok(ToolResult {
                        success: true,
                        data: serde_json::json!({
                            "path": path.display().to_string(),
                            "bytes": bytes,
                            "lines": lines,
                            "mode": if append { "append" } else { "write" }
                        }),
                        message: format!("Fichier écrit: {} ({} octets, {} lignes)",
                            path.display(), bytes, lines),
                    })
                }
                Err(e) => Err(ToolError::ExecutionFailed(format!("Erreur écriture: {}", e))),
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
            "List files in a directory with detailed information"
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "List recursively",
                        "default": false
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum depth for recursive listing",
                        "default": 3
                    }
                },
                "required": ["path"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let path = params["path"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("path is required".to_string()))?;
            let recursive = params["recursive"].as_bool().unwrap_or(false);
            let max_depth = params["max_depth"].as_u64().unwrap_or(3) as usize;
            
            let path = PathBuf::from(path);
            
            if recursive {
                list_recursive(&path, 0, max_depth).await
            } else {
                list_directory(&path).await
            }
        }
    }
    
    async fn list_directory(path: &PathBuf) -> Result<ToolResult, ToolError> {
        match tokio::fs::read_dir(path).await {
            Ok(mut entries) => {
                let mut files = Vec::new();
                while let Some(entry) = entries.next_entry().await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))? 
                {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let metadata = entry.metadata().await.ok();
                    let is_dir = entry.file_type().await.map(|ft| ft.is_dir()).unwrap_or(false);
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    
                    files.push(serde_json::json!({
                        "name": name,
                        "is_directory": is_dir,
                        "size": size,
                    }));
                }
                
                // Sort: directories first, then by name
                files.sort_by(|a, b| {
                    let a_dir = a["is_directory"].as_bool().unwrap_or(false);
                    let b_dir = b["is_directory"].as_bool().unwrap_or(false);
                    match (a_dir, b_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a["name"].as_str().cmp(&b["name"].as_str()),
                    }
                });
                
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({ "files": files }),
                    message: format!("{} éléments dans {}", files.len(), path.display()),
                })
            }
            Err(e) => Err(ToolError::ExecutionFailed(format!("Erreur lecture dossier: {}", e))),
        }
    }
    
    async fn list_recursive(path: &PathBuf, depth: usize, max_depth: usize) -> Result<ToolResult, ToolError> {
        let all_files = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
        collect_files_recursive(path.clone(), all_files.clone(), depth, max_depth).await?;
        
        let files = all_files.lock().await;
        let count = files.len();
        
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({ "files": files.clone() }),
            message: format!("{} fichiers trouvés récursivement", count),
        })
    }
    
    fn collect_files_recursive(
        path: PathBuf,
        files: std::sync::Arc<tokio::sync::Mutex<Vec<Value>>>,
        depth: usize,
        max_depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ToolError>> + Send>> {
        Box::pin(async move {
            if depth > max_depth {
                return Ok(());
            }
            
            let mut entries = tokio::fs::read_dir(&path).await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            
            while let Some(entry) = entries.next_entry().await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            {
                let entry_path = entry.path();
                let name = entry_path.display().to_string();
                let is_dir = entry.file_type().await.map(|ft| ft.is_dir()).unwrap_or(false);
                
                // Skip hidden files and common ignore patterns
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with('.') || 
                   file_name == "node_modules" || 
                   file_name == "target" ||
                   file_name == "__pycache__" {
                    continue;
                }
                
                {
                    let mut files_guard = files.lock().await;
                    files_guard.push(serde_json::json!({
                        "path": name,
                        "is_directory": is_dir,
                        "depth": depth,
                    }));
                }
                
                if is_dir {
                    collect_files_recursive(entry_path, files.clone(), depth + 1, max_depth).await?;
                }
            }
            
            Ok(())
        })
    }
    
    /// Grep tool - search for patterns in files
    pub struct GrepTool;
    
    #[async_trait]
    impl Tool for GrepTool {
        fn name(&self) -> &str {
            "grep"
        }
        
        fn description(&self) -> &str {
            "Search for a pattern in files using regex. Returns matching lines with context."
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search in"
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "description": "Case insensitive search",
                        "default": false
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Lines of context before and after match",
                        "default": 2
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results",
                        "default": 50
                    }
                },
                "required": ["pattern", "path"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let pattern = params["pattern"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("pattern is required".to_string()))?;
            let path = params["path"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("path is required".to_string()))?;
            let case_insensitive = params["case_insensitive"].as_bool().unwrap_or(false);
            let context_lines = params["context_lines"].as_u64().unwrap_or(2) as usize;
            let max_results = params["max_results"].as_u64().unwrap_or(50) as usize;
            
            let regex_pattern = if case_insensitive {
                format!("(?i){}", pattern)
            } else {
                pattern.to_string()
            };
            
            let regex = Regex::new(&regex_pattern)
                .map_err(|e| ToolError::InvalidParameters(format!("Invalid regex: {}", e)))?;
            
            let path = PathBuf::from(path);
            
            if path.is_file() {
                let mut results = Vec::new();
                let mut total_matches = 0;
                search_file(&path, &regex, context_lines, &mut results, &mut total_matches, max_results).await?;
                
                let truncated = total_matches > max_results;
                
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({
                        "matches": results,
                        "total_matches": total_matches,
                        "truncated": truncated
                    }),
                    message: format!("{} correspondance(s) trouvée(s){}", 
                        total_matches,
                        if truncated { " (résultats tronqués)" } else { "" }),
                })
            } else if path.is_dir() {
                let results = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
                let total_matches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let regex = std::sync::Arc::new(regex);
                
                search_directory(path, regex, context_lines, results.clone(), total_matches.clone(), max_results).await?;
                
                let results_vec = results.lock().await;
                let total = total_matches.load(std::sync::atomic::Ordering::Relaxed);
                let truncated = total > max_results;
                
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({
                        "matches": results_vec.clone(),
                        "total_matches": total,
                        "truncated": truncated
                    }),
                    message: format!("{} correspondance(s) trouvée(s){}", 
                        total,
                        if truncated { " (résultats tronqués)" } else { "" }),
                })
            } else {
                Err(ToolError::InvalidParameters("Path does not exist".to_string()))
            }
        }
    }
    
    async fn search_file(
        path: &PathBuf,
        regex: &Regex,
        context_lines: usize,
        results: &mut Vec<Value>,
        total_matches: &mut usize,
        max_results: usize,
    ) -> Result<(), ToolError> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => return Ok(()), // Skip unreadable files
        };
        
        let lines: Vec<&str> = content.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                *total_matches += 1;
                
                if results.len() < max_results {
                    let start = i.saturating_sub(context_lines);
                    let end = (i + context_lines + 1).min(lines.len());
                    
                    let context: Vec<String> = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(j, l)| {
                            let line_num = start + j + 1;
                            let marker = if start + j == i { ">" } else { " " };
                            format!("{}{:>4}| {}", marker, line_num, l)
                        })
                        .collect();
                    
                    results.push(serde_json::json!({
                        "file": path.display().to_string(),
                        "line": i + 1,
                        "content": line,
                        "context": context.join("\n")
                    }));
                }
            }
        }
        
        Ok(())
    }
    
    async fn search_file_async(
        path: &PathBuf,
        regex: &Regex,
        context_lines: usize,
        results: &std::sync::Arc<tokio::sync::Mutex<Vec<Value>>>,
        total_matches: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
        max_results: usize,
    ) -> Result<(), ToolError> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => return Ok(()), // Skip unreadable files
        };
        
        let lines: Vec<&str> = content.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                total_matches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                
                let mut results_guard = results.lock().await;
                if results_guard.len() < max_results {
                    let start = i.saturating_sub(context_lines);
                    let end = (i + context_lines + 1).min(lines.len());
                    
                    let context: Vec<String> = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(j, l)| {
                            let line_num = start + j + 1;
                            let marker = if start + j == i { ">" } else { " " };
                            format!("{}{:>4}| {}", marker, line_num, l)
                        })
                        .collect();
                    
                    results_guard.push(serde_json::json!({
                        "file": path.display().to_string(),
                        "line": i + 1,
                        "content": line,
                        "context": context.join("\n")
                    }));
                }
            }
        }
        
        Ok(())
    }
    
    fn search_directory(
        path: PathBuf,
        regex: std::sync::Arc<Regex>,
        context_lines: usize,
        results: std::sync::Arc<tokio::sync::Mutex<Vec<Value>>>,
        total_matches: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        max_results: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ToolError>> + Send>> {
        Box::pin(async move {
            let mut entries = match tokio::fs::read_dir(&path).await {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };
            
            while let Ok(Some(entry)) = entries.next_entry().await {
                {
                    let results_guard = results.lock().await;
                    if results_guard.len() >= max_results {
                        break;
                    }
                }
                
                let entry_path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Skip hidden and common ignore patterns
                if name.starts_with('.') || 
                   name == "node_modules" || 
                   name == "target" ||
                   name == "__pycache__" ||
                   name.ends_with(".lock") {
                    continue;
                }
                
                if entry_path.is_file() {
                    // Only search text files
                    let ext = entry_path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let text_extensions = ["rs", "py", "js", "ts", "tsx", "jsx", "json", "toml", 
                        "yaml", "yml", "md", "txt", "html", "css", "scss", "vue", "go", "java", 
                        "c", "cpp", "h", "hpp", "sh", "bash", "zsh"];
                    
                    if text_extensions.contains(&ext) || ext.is_empty() {
                        search_file_async(&entry_path, &regex, context_lines, &results, &total_matches, max_results).await?;
                    }
                } else if entry_path.is_dir() {
                    search_directory(entry_path, regex.clone(), context_lines, results.clone(), total_matches.clone(), max_results).await?;
                }
            }
            
            Ok(())
        })
    }
    
    /// Glob tool - find files by pattern
    pub struct GlobTool;
    
    #[async_trait]
    impl Tool for GlobTool {
        fn name(&self) -> &str {
            "glob"
        }
        
        fn description(&self) -> &str {
            "Find files matching a glob pattern (e.g., '**/*.rs', 'src/**/*.py')"
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern (e.g., '**/*.rs', 'src/**/*.py')"
                    },
                    "base_path": {
                        "type": "string",
                        "description": "Base directory to search from (default: current dir)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results",
                        "default": 100
                    }
                },
                "required": ["pattern"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let pattern = params["pattern"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("pattern is required".to_string()))?;
            let base_path = params["base_path"].as_str().unwrap_or(".");
            let max_results = params["max_results"].as_u64().unwrap_or(100) as usize;
            
            let full_pattern = if pattern.starts_with('/') || pattern.starts_with("C:") {
                pattern.to_string()
            } else {
                format!("{}/{}", base_path, pattern)
            };
            
            let mut files = Vec::new();
            
            match glob_match(&full_pattern) {
                Ok(paths) => {
                    for entry in paths.take(max_results) {
                        match entry {
                            Ok(path) => {
                                let is_dir = path.is_dir();
                                files.push(serde_json::json!({
                                    "path": path.display().to_string(),
                                    "is_directory": is_dir,
                                }));
                            }
                            Err(_) => continue,
                        }
                    }
                }
                Err(e) => {
                    return Err(ToolError::InvalidParameters(format!("Invalid glob pattern: {}", e)));
                }
            }
            
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({ "files": files }),
                message: format!("{} fichier(s) trouvé(s) pour '{}'", files.len(), pattern),
            })
        }
    }
    
    /// Think tool - for explicit reasoning steps
    pub struct ThinkTool;
    
    #[async_trait]
    impl Tool for ThinkTool {
        fn name(&self) -> &str {
            "think"
        }
        
        fn description(&self) -> &str {
            "Use this to record your reasoning process. Helps you think through complex problems step by step."
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "thought": {
                        "type": "string",
                        "description": "Your current reasoning or analysis"
                    }
                },
                "required": ["thought"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let thought = params["thought"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("thought is required".to_string()))?;
            
            // The think tool just acknowledges the thought
            // The actual value is in recording it in the context
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({
                    "thought": thought,
                    "recorded": true
                }),
                message: "Réflexion enregistrée. Continue ton raisonnement.".to_string(),
            })
        }
    }
    
    /// TodoWrite tool - for managing task lists
    pub struct TodoWriteTool;
    
    #[async_trait]
    impl Tool for TodoWriteTool {
        fn name(&self) -> &str {
            "todo_write"
        }
        
        fn description(&self) -> &str {
            "Create or update a task list to plan your work. Use for complex multi-step tasks."
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "description": "Array of TODO items",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "Unique identifier"
                                },
                                "content": {
                                    "type": "string",
                                    "description": "Task description"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"],
                                    "description": "Task status"
                                }
                            },
                            "required": ["id", "content", "status"]
                        }
                    },
                    "merge": {
                        "type": "boolean",
                        "description": "Merge with existing todos (true) or replace (false)",
                        "default": true
                    }
                },
                "required": ["todos"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let todos = params.get("todos")
                .ok_or_else(|| ToolError::InvalidParameters("todos is required".to_string()))?;
            
            // Validate todos array
            let todos_arr = todos.as_array()
                .ok_or_else(|| ToolError::InvalidParameters("todos must be an array".to_string()))?;
            
            let mut valid_todos = Vec::new();
            for todo in todos_arr {
                let id = todo.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let content = todo.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
                
                if !id.is_empty() && !content.is_empty() {
                    valid_todos.push(serde_json::json!({
                        "id": id,
                        "content": content,
                        "status": status
                    }));
                }
            }
            
            let pending = valid_todos.iter()
                .filter(|t| t["status"] == "pending")
                .count();
            let in_progress = valid_todos.iter()
                .filter(|t| t["status"] == "in_progress")
                .count();
            let completed = valid_todos.iter()
                .filter(|t| t["status"] == "completed")
                .count();
            
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({
                    "todos": valid_todos,
                    "stats": {
                        "total": valid_todos.len(),
                        "pending": pending,
                        "in_progress": in_progress,
                        "completed": completed
                    }
                }),
                message: format!(
                    "Plan mis à jour: {} tâches ({} en attente, {} en cours, {} terminées)",
                    valid_todos.len(), pending, in_progress, completed
                ),
            })
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
            "Execute a shell command (requires approval). Only safe read-only commands allowed."
        }
        
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Working directory (optional)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds",
                        "default": 30
                    }
                },
                "required": ["command"]
            })
        }
        
        async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
            let command_str = params["command"].as_str()
                .ok_or_else(|| ToolError::InvalidParameters("command is required".to_string()))?;
            let working_dir = params["working_dir"].as_str();
            let timeout_secs = params["timeout_secs"].as_u64().unwrap_or(30);
            
            // SECURITY: Only allow safe read-only commands
            let allowed_commands = [
                "ls", "dir", "cat", "type", "echo", "pwd", "cd", 
                "whoami", "date", "wc", "head", "tail", "find", 
                "grep", "rg", "tree", "which", "where", "env", "mkdir"
            ];
            
            let cmd_parts: Vec<&str> = command_str.split_whitespace().collect();
            if cmd_parts.is_empty() {
                return Err(ToolError::InvalidParameters("Empty command".to_string()));
            }
            
            let base_cmd = cmd_parts[0].split('/').last().unwrap_or(cmd_parts[0]);
            if !allowed_commands.contains(&base_cmd) {
                return Err(ToolError::PermissionDenied(
                    format!("Commande '{}' non autorisée. Commandes permises: {:?}", 
                        base_cmd, allowed_commands)
                ));
            }
            
            // Build command
            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let shell_arg = if cfg!(windows) { "/C" } else { "-c" };
            
            let mut cmd = Command::new(shell);
            cmd.arg(shell_arg).arg(command_str);
            
            if let Some(dir) = working_dir {
                cmd.current_dir(dir);
            }
            
            // Execute with timeout
            let result = timeout(
                Duration::from_secs(timeout_secs),
                cmd.output()
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
                            "Commande exécutée".to_string()
                        } else {
                            format!("Commande échouée (code: {:?})", output.status.code())
                        },
                    })
                }
                Ok(Err(e)) => Err(ToolError::ExecutionFailed(format!("Erreur exécution: {}", e))),
                Err(_) => Err(ToolError::Timeout),
            }
        }
    }
}
