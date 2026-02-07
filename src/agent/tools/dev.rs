//! Developer tools - Diff, Find & Replace, Patch
//!
//! Provides developer-oriented utilities for code manipulation.

use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// DiffTool - Compare two files or strings
// ============================================================================

pub struct DiffTool;

#[async_trait]
impl Tool for DiffTool {
    fn name(&self) -> &str {
        "diff"
    }

    fn description(&self) -> &str {
        "Compare two files or text strings and show the differences line by line."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_a": {
                    "type": "string",
                    "description": "First file path (or use text_a for inline text)"
                },
                "file_b": {
                    "type": "string",
                    "description": "Second file path (or use text_b for inline text)"
                },
                "text_a": {
                    "type": "string",
                    "description": "First text to compare (alternative to file_a)"
                },
                "text_b": {
                    "type": "string",
                    "description": "Second text to compare (alternative to file_b)"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Lines of context around changes (default: 3)",
                    "default": 3
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let context_lines = params["context_lines"].as_u64().unwrap_or(3) as usize;

        let text_a = if let Some(path) = params["file_a"].as_str() {
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de lire {}: {}", path, e)))?
        } else if let Some(text) = params["text_a"].as_str() {
            text.to_string()
        } else {
            return Err(ToolError::InvalidParameters(
                "file_a or text_a is required".into(),
            ));
        };

        let text_b = if let Some(path) = params["file_b"].as_str() {
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de lire {}: {}", path, e)))?
        } else if let Some(text) = params["text_b"].as_str() {
            text.to_string()
        } else {
            return Err(ToolError::InvalidParameters(
                "file_b or text_b is required".into(),
            ));
        };

        let lines_a: Vec<&str> = text_a.lines().collect();
        let lines_b: Vec<&str> = text_b.lines().collect();

        // Simple line-by-line diff
        let diff = compute_line_diff(&lines_a, &lines_b, context_lines);
        let changes = diff
            .iter()
            .filter(|l| l.starts_with('+') || l.starts_with('-'))
            .count();

        let label_a = params["file_a"]
            .as_str()
            .unwrap_or("text_a")
            .to_string();
        let label_b = params["file_b"]
            .as_str()
            .unwrap_or("text_b")
            .to_string();

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "diff": diff.join("\n"),
                "changes": changes,
                "lines_a": lines_a.len(),
                "lines_b": lines_b.len(),
                "label_a": label_a,
                "label_b": label_b,
            }),
            message: format!(
                "Diff: {} changement(s) entre {} ({} lignes) et {} ({} lignes)",
                changes,
                label_a,
                lines_a.len(),
                label_b,
                lines_b.len()
            ),
        })
    }
}

// ============================================================================
// FindReplaceTool - Find and replace across multiple files
// ============================================================================

pub struct FindReplaceTool;

#[async_trait]
impl Tool for FindReplaceTool {
    fn name(&self) -> &str {
        "find_replace"
    }

    fn description(&self) -> &str {
        "Find and replace text across multiple files in a directory. Supports file pattern filtering. REQUIRES APPROVAL."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "search": {
                    "type": "string",
                    "description": "Text to search for"
                },
                "replace": {
                    "type": "string",
                    "description": "Replacement text"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "File extension filter (e.g., 'rs', 'py', 'js')"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "Preview changes without applying (default: false)",
                    "default": false
                },
                "max_files": {
                    "type": "integer",
                    "description": "Maximum files to modify (default: 50)",
                    "default": 50
                }
            },
            "required": ["search", "replace", "path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let search = params["search"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("search is required".into()))?;
        let replace = params["replace"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("replace is required".into()))?;
        let path = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;
        let file_pattern = params["file_pattern"].as_str();
        let dry_run = params["dry_run"].as_bool().unwrap_or(false);
        let max_files = params["max_files"].as_u64().unwrap_or(50) as usize;

        let path_buf = PathBuf::from(path);
        let mut modified_files = Vec::new();
        let mut total_replacements = 0usize;

        find_replace_recursive(
            &path_buf,
            search,
            replace,
            file_pattern,
            dry_run,
            &mut modified_files,
            &mut total_replacements,
            max_files,
        )
        .await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "files_modified": modified_files,
                "total_replacements": total_replacements,
                "dry_run": dry_run,
                "search": search,
                "replace": replace
            }),
            message: format!(
                "{}{} remplacement(s) dans {} fichier(s)",
                if dry_run { "[DRY RUN] " } else { "" },
                total_replacements,
                modified_files.len()
            ),
        })
    }
}

fn find_replace_recursive<'a>(
    path: &'a PathBuf,
    search: &'a str,
    replace: &'a str,
    file_pattern: Option<&'a str>,
    dry_run: bool,
    modified_files: &'a mut Vec<Value>,
    total_replacements: &'a mut usize,
    max_files: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ToolError>> + Send + 'a>> {
    Box::pin(async move {
        if modified_files.len() >= max_files {
            return Ok(());
        }

        if path.is_file() {
            if let Some(pattern) = file_pattern {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext != pattern {
                    return Ok(());
                }
            }

            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let count = content.matches(search).count();
                if count > 0 {
                    *total_replacements += count;
                    modified_files.push(serde_json::json!({
                        "file": path.display().to_string(),
                        "replacements": count
                    }));

                    if !dry_run {
                        let new_content = content.replace(search, replace);
                        tokio::fs::write(path, new_content)
                            .await
                            .map_err(|e| {
                                ToolError::ExecutionFailed(format!(
                                    "Impossible d'écrire {}: {}",
                                    path.display(),
                                    e
                                ))
                            })?;
                    }
                }
            }
        } else if path.is_dir() {
            let mut entries = match tokio::fs::read_dir(path).await {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                if modified_files.len() >= max_files {
                    break;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "target"
                    || name == "__pycache__"
                    || name == ".git"
                {
                    continue;
                }
                find_replace_recursive(
                    &entry.path(),
                    search,
                    replace,
                    file_pattern,
                    dry_run,
                    modified_files,
                    total_replacements,
                    max_files,
                )
                .await?;
            }
        }

        Ok(())
    })
}

// ============================================================================
// PatchTool - Apply unified diff patches
// ============================================================================

pub struct PatchTool;

#[async_trait]
impl Tool for PatchTool {
    fn name(&self) -> &str {
        "patch"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch to a file. The patch should be in unified diff format. REQUIRES APPROVAL."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File to patch"
                },
                "patch": {
                    "type": "string",
                    "description": "Unified diff patch content"
                }
            },
            "required": ["path", "patch"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let path = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;
        let patch = params["patch"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("patch is required".into()))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de lire: {}", e)))?;

        let new_content = apply_simple_patch(&content, patch)?;

        tokio::fs::write(path, &new_content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Impossible d'écrire: {}", e)))?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": path,
                "lines_before": content.lines().count(),
                "lines_after": new_content.lines().count()
            }),
            message: format!("Patch appliqué à {}", path),
        })
    }
}

// ============================================================================
// CountLinesTool - Count lines, words, chars in files
// ============================================================================

pub struct CountLinesTool;

#[async_trait]
impl Tool for CountLinesTool {
    fn name(&self) -> &str {
        "wc"
    }

    fn description(&self) -> &str {
        "Count lines, words, and characters in a file (like the wc command)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let path = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de lire: {}", e)))?;

        let lines = content.lines().count();
        let words = content.split_whitespace().count();
        let chars = content.chars().count();
        let bytes = content.len();

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": path,
                "lines": lines,
                "words": words,
                "characters": chars,
                "bytes": bytes
            }),
            message: format!("{}: {} lignes, {} mots, {} caractères", path, lines, words, chars),
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Simple line-by-line diff with context
fn compute_line_diff(lines_a: &[&str], lines_b: &[&str], context: usize) -> Vec<String> {
    let mut result = Vec::new();
    let max_len = lines_a.len().max(lines_b.len());
    let mut changes: Vec<(usize, String)> = Vec::new();

    // Find all changed lines
    let mut i = 0;
    let mut j = 0;
    while i < lines_a.len() || j < lines_b.len() {
        match (lines_a.get(i), lines_b.get(j)) {
            (Some(a), Some(b)) if a == b => {
                changes.push((i, format!(" {}", a)));
                i += 1;
                j += 1;
            }
            (Some(a), Some(_b)) => {
                changes.push((i, format!("-{}", a)));
                i += 1;
                // Don't advance j - will be picked up as addition
                if j < lines_b.len() {
                    changes.push((max_len + j, format!("+{}", lines_b[j])));
                    j += 1;
                }
            }
            (Some(a), None) => {
                changes.push((i, format!("-{}", a)));
                i += 1;
            }
            (None, Some(b)) => {
                changes.push((max_len + j, format!("+{}", b)));
                j += 1;
            }
            (None, None) => break,
        }
    }

    // Filter with context
    let change_indices: Vec<usize> = changes
        .iter()
        .enumerate()
        .filter(|(_, (_, line))| line.starts_with('+') || line.starts_with('-'))
        .map(|(idx, _)| idx)
        .collect();

    if change_indices.is_empty() {
        result.push("Aucune différence trouvée.".to_string());
        return result;
    }

    let mut shown = vec![false; changes.len()];
    for &idx in &change_indices {
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(changes.len());
        for k in start..end {
            shown[k] = true;
        }
    }

    let mut prev_shown = false;
    for (idx, (_, line)) in changes.iter().enumerate() {
        if shown[idx] {
            if !prev_shown && idx > 0 {
                result.push("---".to_string());
            }
            result.push(line.clone());
            prev_shown = true;
        } else {
            prev_shown = false;
        }
    }

    result
}

/// Apply a simple unified diff patch
fn apply_simple_patch(content: &str, patch: &str) -> Result<String, ToolError> {
    let _lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Simplified patch: apply line-by-line additions and removals
    let mut result_lines: Vec<String> = Vec::new();
    let mut content_iter = content.lines().peekable();
    let mut patch_iter = patch.lines().peekable();

    // Skip header lines (---, +++, etc.)
    while let Some(line) = patch_iter.peek() {
        if line.starts_with("---") || line.starts_with("+++") || line.starts_with("@@") {
            patch_iter.next();
        } else {
            break;
        }
    }

    for patch_line in patch_iter {
        if patch_line.starts_with('+') {
            // Addition
            result_lines.push(patch_line[1..].to_string());
        } else if patch_line.starts_with('-') {
            // Removal - skip the corresponding line from content
            content_iter.next();
        } else if patch_line.starts_with(' ') {
            // Context line
            if let Some(line) = content_iter.next() {
                result_lines.push(line.to_string());
            }
        } else if patch_line.starts_with("@@") {
            // Hunk header - skip
            continue;
        }
    }

    // Add remaining lines
    for line in content_iter {
        result_lines.push(line.to_string());
    }

    Ok(result_lines.join("\n"))
}
