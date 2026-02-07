//! Agent helper utilities
//!
//! Provides tool call parsing and prompt helpers.

use serde_json::Value;

use crate::agent::tools::{ToolInfo, ToolResult};
use crate::agent::permissions::PermissionLevel;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool: String,
    pub params: Value,
}

pub fn tool_permission_level(tool_name: &str) -> PermissionLevel {
    match tool_name {
        // Read-only tools
        "file_read" | "file_list" | "grep" | "glob" | "think" | "todo_write"
        | "file_info" | "file_search" | "diff" | "wc" | "tree"
        | "process_list" | "environment" | "system_info" | "which"
        | "git_status" | "git_diff" | "git_log" | "git_branch" => {
            PermissionLevel::ReadOnly
        }
        // Network tools
        "web_search" | "code_search" | "company_research"
        | "deep_research_start" | "deep_research_check" | "web_crawl"
        | "web_fetch" | "web_download" => {
            PermissionLevel::Network
        }
        // File write tools
        "file_write" | "file_edit" | "file_create" | "file_delete"
        | "file_move" | "file_copy" | "directory_create"
        | "find_replace" | "patch" => {
            PermissionLevel::WriteFile
        }
        // Safe command execution
        "command" => PermissionLevel::ExecuteSafe,
        // Unsafe execution
        "bash" | "bash_background" | "git_commit" | "git_stash" => {
            PermissionLevel::ExecuteUnsafe
        }
        // MCP tools
        name if name.starts_with("mcp_") => PermissionLevel::Network,
        _ => PermissionLevel::ReadOnly,
    }
}

pub fn build_tool_instructions(tools: &[ToolInfo]) -> String {
    if tools.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "## Tools\n\
If you need to use a tool, respond ONLY with a JSON object in this format:\n\
{\"tool\":\"tool_name\",\"params\":{...}}\n\
Do not add any extra text.\n\
\n\
Available tools:\n",
    );

    for tool in tools {
        let schema = serde_json::to_string(&tool.parameters_schema)
            .unwrap_or_else(|_| "{}".to_string());
        out.push_str("- ");
        out.push_str(&tool.name);
        out.push_str(": ");
        out.push_str(&tool.description);
        out.push_str("\n  params_schema: ");
        out.push_str(&schema);
        out.push('\n');
    }

    out
}

pub fn format_tool_result_for_system(tool: &str, result: &ToolResult) -> String {
    let data = serde_json::to_string(&result.data).unwrap_or_else(|_| "{}".to_string());
    format!(
        "{{\"tool\":\"{}\",\"success\":{},\"message\":{},\"data\":{}}}",
        tool,
        result.success,
        serde_json::to_string(&result.message).unwrap_or_else(|_| "\"\"".to_string()),
        data
    )
}

pub fn extract_tool_call(text: &str) -> Option<ToolCall> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(call) = parse_tool_call_json(trimmed) {
        return Some(call);
    }

    if let Some(code_block) = extract_code_block(trimmed) {
        if let Some(call) = parse_tool_call_json(code_block) {
            return Some(call);
        }
    }

    if let Some(json_block) = extract_json_object(trimmed) {
        if let Some(call) = parse_tool_call_json(&json_block) {
            return Some(call);
        }
    }

    None
}

fn parse_tool_call_json(input: &str) -> Option<ToolCall> {
    let value: Value = serde_json::from_str(input).ok()?;
    let obj = value.as_object()?;
    let tool = obj.get("tool").and_then(|v| v.as_str())?.to_string();
    let params = obj
        .get("params")
        .cloned()
        .or_else(|| obj.get("arguments").cloned())
        .unwrap_or(Value::Null);

    Some(ToolCall { tool, params })
}

fn extract_code_block(text: &str) -> Option<&str> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let after_lang = if let Some(newline) = rest.find('\n') {
        &rest[newline + 1..]
    } else {
        rest
    };
    let end = after_lang.find("```")?;
    Some(&after_lang[..end])
}

fn extract_json_object(text: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut start: Option<usize> = None;

    for (i, ch) in text.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    if let Some(start_idx) = start {
                        return Some(text[start_idx..=i].to_string());
                    }
                }
            }
            _ => {}
        }
    }

    None
}
