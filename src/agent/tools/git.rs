//! Git tools - Status, Diff, Log, Commit, Branch operations
//!
//! Provides git operations by shelling out to the git CLI.

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::agent::tools::{Tool, ToolError, ToolResult};

/// Helper to run git commands
async fn run_git(args: &[&str], working_dir: Option<&str>) -> Result<(String, String, bool), ToolError> {
    let mut cmd = Command::new("git");
    for arg in args {
        cmd.arg(arg);
    }
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Impossible d'exécuter git: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((stdout, stderr, output.status.success()))
}

// ============================================================================
// GitStatusTool
// ============================================================================

pub struct GitStatusTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str { "git_status" }

    fn description(&self) -> &str {
        "Get the current git status: modified files, staged changes, branch info, untracked files."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "working_dir": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let wd = params["working_dir"].as_str();

        let (status_out, _, _) = run_git(&["status", "--porcelain=v2", "--branch"], wd).await?;
        let (status_short, _, _) = run_git(&["status", "--short"], wd).await?;
        let (branch, _, _) = run_git(&["branch", "--show-current"], wd).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "branch": branch.trim(),
                "status_porcelain": status_out,
                "status_short": status_short,
            }),
            message: format!("Branche: {} | {} fichier(s) modifié(s)", 
                branch.trim(), 
                status_short.lines().count()),
        })
    }
}

// ============================================================================
// GitDiffTool
// ============================================================================

pub struct GitDiffTool;

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str { "git_diff" }

    fn description(&self) -> &str {
        "Show git diff - unstaged changes, staged changes, or diff between commits/branches."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "staged": {
                    "type": "boolean",
                    "description": "Show staged changes (--cached)",
                    "default": false
                },
                "file": {
                    "type": "string",
                    "description": "Specific file to diff (optional)"
                },
                "ref1": {
                    "type": "string",
                    "description": "First ref (commit/branch) for comparison"
                },
                "ref2": {
                    "type": "string",
                    "description": "Second ref for comparison"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Repository path"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let staged = params["staged"].as_bool().unwrap_or(false);
        let file = params["file"].as_str();
        let ref1 = params["ref1"].as_str();
        let ref2 = params["ref2"].as_str();
        let wd = params["working_dir"].as_str();

        let mut args: Vec<&str> = vec!["diff", "--stat"];
        if staged {
            args.push("--cached");
        }
        if let Some(r1) = ref1 {
            args.push(r1);
        }
        if let Some(r2) = ref2 {
            args.push(r2);
        }
        if let Some(f) = file {
            args.push("--");
            args.push(f);
        }

        let (stat_out, _, _) = run_git(&args, wd).await?;

        // Also get the actual diff (without --stat)
        args.retain(|a| *a != "--stat");
        let (diff_out, _, _) = run_git(&args, wd).await?;

        // Truncate if too long (safe char-boundary slicing)
        let diff_display = if diff_out.len() > 50000 {
            let safe = crate::truncate_str(&diff_out, 50000);
            format!("{}...\n[truncated, diff too long]", safe)
        } else {
            diff_out
        };

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "stat": stat_out,
                "diff": diff_display,
            }),
            message: format!("Diff: {} fichier(s) modifié(s)", stat_out.lines().count().saturating_sub(1)),
        })
    }
}

// ============================================================================
// GitLogTool
// ============================================================================

pub struct GitLogTool;

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str { "git_log" }

    fn description(&self) -> &str {
        "Show git commit history with messages, authors, and dates."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of commits to show (default: 10)",
                    "default": 10
                },
                "oneline": {
                    "type": "boolean",
                    "description": "Show one line per commit (default: true)",
                    "default": true
                },
                "file": {
                    "type": "string",
                    "description": "Show history for a specific file"
                },
                "author": {
                    "type": "string",
                    "description": "Filter by author"
                },
                "since": {
                    "type": "string",
                    "description": "Show commits after date (e.g., '2024-01-01', '1 week ago')"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Repository path"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let count = params["count"].as_u64().unwrap_or(10);
        let oneline = params["oneline"].as_bool().unwrap_or(true);
        let file = params["file"].as_str();
        let author = params["author"].as_str();
        let since = params["since"].as_str();
        let wd = params["working_dir"].as_str();

        let count_str = count.to_string();
        let mut args = vec!["log", "-n", &count_str];

        let format_str;
        if oneline {
            format_str = "--pretty=format:%h %ad %an: %s".to_string();
            args.push(&format_str);
            args.push("--date=short");
        } else {
            format_str = "--pretty=format:%H%n%ad %an <%ae>%n%s%n%b%n---".to_string();
            args.push(&format_str);
            args.push("--date=iso");
        }

        let author_filter;
        if let Some(a) = author {
            author_filter = format!("--author={}", a);
            args.push(&author_filter);
        }

        let since_filter;
        if let Some(s) = since {
            since_filter = format!("--since={}", s);
            args.push(&since_filter);
        }

        if let Some(f) = file {
            args.push("--");
            args.push(f);
        }

        let (log_out, stderr, success) = run_git(&args, wd).await?;

        if !success {
            return Err(ToolError::ExecutionFailed(format!("git log failed: {}", stderr)));
        }

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "log": log_out,
                "count": log_out.lines().filter(|l| !l.is_empty()).count()
            }),
            message: format!("{} commit(s) affichés", count),
        })
    }
}

// ============================================================================
// GitCommitTool
// ============================================================================

pub struct GitCommitTool;

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str { "git_commit" }

    fn description(&self) -> &str {
        "Stage files and create a git commit. Can stage specific files or all changes. REQUIRES APPROVAL."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Commit message"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files to stage (omit for 'git add -A')"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Repository path"
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let message = params["message"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("message is required".into()))?;
        let files = params["files"].as_array();
        let wd = params["working_dir"].as_str();

        // Stage files
        if let Some(file_list) = files {
            for f in file_list {
                if let Some(file_path) = f.as_str() {
                    let (_, stderr, success) = run_git(&["add", file_path], wd).await?;
                    if !success {
                        return Err(ToolError::ExecutionFailed(format!(
                            "git add failed for {}: {}",
                            file_path, stderr
                        )));
                    }
                }
            }
        } else {
            let (_, stderr, success) = run_git(&["add", "-A"], wd).await?;
            if !success {
                return Err(ToolError::ExecutionFailed(format!("git add -A failed: {}", stderr)));
            }
        }

        // Commit
        let (stdout, stderr, success) = run_git(&["commit", "-m", message], wd).await?;
        if !success {
            return Err(ToolError::ExecutionFailed(format!(
                "git commit failed: {}",
                stderr
            )));
        }

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "message": message,
                "output": stdout.trim(),
            }),
            message: format!("Commit créé: {}", message),
        })
    }
}

// ============================================================================
// GitBranchTool
// ============================================================================

pub struct GitBranchTool;

#[async_trait]
impl Tool for GitBranchTool {
    fn name(&self) -> &str { "git_branch" }

    fn description(&self) -> &str {
        "List, create, switch, or delete git branches."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "switch", "delete"],
                    "description": "Action to perform (default: list)",
                    "default": "list"
                },
                "name": {
                    "type": "string",
                    "description": "Branch name (required for create/switch/delete)"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Repository path"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let action = params["action"].as_str().unwrap_or("list");
        let name = params["name"].as_str();
        let wd = params["working_dir"].as_str();

        match action {
            "list" => {
                let (stdout, _, _) = run_git(&["branch", "-a", "--no-color"], wd).await?;
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({ "branches": stdout }),
                    message: format!("{} branche(s)", stdout.lines().count()),
                })
            }
            "create" => {
                let branch = name.ok_or_else(|| ToolError::InvalidParameters("name is required for create".into()))?;
                let (_, stderr, success) = run_git(&["checkout", "-b", branch], wd).await?;
                if !success {
                    return Err(ToolError::ExecutionFailed(format!("Impossible de créer la branche: {}", stderr)));
                }
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({ "branch": branch, "action": "created" }),
                    message: format!("Branche créée et activée: {}", branch),
                })
            }
            "switch" => {
                let branch = name.ok_or_else(|| ToolError::InvalidParameters("name is required for switch".into()))?;
                let (_, stderr, success) = run_git(&["checkout", branch], wd).await?;
                if !success {
                    return Err(ToolError::ExecutionFailed(format!("Impossible de changer de branche: {}", stderr)));
                }
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({ "branch": branch, "action": "switched" }),
                    message: format!("Basculé sur la branche: {}", branch),
                })
            }
            "delete" => {
                let branch = name.ok_or_else(|| ToolError::InvalidParameters("name is required for delete".into()))?;
                let (_, stderr, success) = run_git(&["branch", "-d", branch], wd).await?;
                if !success {
                    return Err(ToolError::ExecutionFailed(format!("Impossible de supprimer la branche: {}", stderr)));
                }
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({ "branch": branch, "action": "deleted" }),
                    message: format!("Branche supprimée: {}", branch),
                })
            }
            _ => Err(ToolError::InvalidParameters(format!("Action inconnue: {}", action))),
        }
    }
}

// ============================================================================
// GitStashTool
// ============================================================================

pub struct GitStashTool;

#[async_trait]
impl Tool for GitStashTool {
    fn name(&self) -> &str { "git_stash" }

    fn description(&self) -> &str {
        "Stash or restore uncommitted changes. Actions: save, pop, list, drop."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["save", "pop", "list", "drop"],
                    "description": "Stash action (default: save)",
                    "default": "save"
                },
                "message": {
                    "type": "string",
                    "description": "Stash message (for save action)"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Repository path"
                }
            }
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let action = params["action"].as_str().unwrap_or("save");
        let message = params["message"].as_str();
        let wd = params["working_dir"].as_str();

        let (stdout, stderr, success) = match action {
            "save" => {
                if let Some(msg) = message {
                    run_git(&["stash", "push", "-m", msg], wd).await?
                } else {
                    run_git(&["stash", "push"], wd).await?
                }
            }
            "pop" => run_git(&["stash", "pop"], wd).await?,
            "list" => run_git(&["stash", "list"], wd).await?,
            "drop" => run_git(&["stash", "drop"], wd).await?,
            _ => return Err(ToolError::InvalidParameters(format!("Action inconnue: {}", action))),
        };

        if !success && action != "list" {
            return Err(ToolError::ExecutionFailed(format!("git stash {} failed: {}", action, stderr)));
        }

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "action": action,
                "output": stdout.trim(),
            }),
            message: format!("git stash {}: {}", action, stdout.trim()),
        })
    }
}
