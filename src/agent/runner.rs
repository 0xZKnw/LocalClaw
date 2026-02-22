//! Agent helper utilities
//!
//! Provides tool call parsing and prompt helpers.

use regex::Regex;
use serde_json::Value;

use crate::agent::tools::{ToolInfo, ToolResult};

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool: String,
    pub params: Value,
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
        let schema =
            serde_json::to_string(&tool.parameters_schema).unwrap_or_else(|_| "{}".to_string());
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
    // For skills, use a more readable format since output is the key data
    if tool.starts_with("skill_") {
        return format!(
            "<tool_result>\n<tool>{}</tool>\n<success>{}</success>\n<output>\n{}\n</output>\n</tool_result>",
            tool,
            result.success,
            result.message
        );
    }

    // Standard compact format for other tools
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

    // Try 0: XML-style parsing (Robust for multi-line content)
    if let Some(call) = extract_xml_tool_call(trimmed) {
        return Some(call);
    }

    // Try 1: Direct JSON parse
    if let Some(call) = parse_tool_call_json(trimmed) {
        return Some(call);
    }

    // Try 2: Extract from code block
    if let Some(code_block) = extract_code_block(trimmed) {
        if let Some(call) = parse_tool_call_json(code_block) {
            return Some(call);
        }
    }

    // Try 3: Find ALL JSON objects in the text and check each for "tool" field
    // OR try heuristic detection if the JSON is just the params
    for json_block in extract_all_json_objects(trimmed) {
        if let Some(call) = parse_tool_call_json(&json_block) {
            return Some(call);
        }

        // Heuristic fallback: check if JSON looks like params for specific tools
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_block) {
            if let Some(obj) = value.as_object() {
                // Heuristic for skill_create
                if obj.contains_key("name")
                    && obj.contains_key("description")
                    && obj.contains_key("content")
                {
                    // Check if it's NOT a nested object inside params (which would be handled above)
                    // This is for when the model outputs JUST the params
                    tracing::warn!(
                        "Heuristic: detected potential skill_create params without wrapper"
                    );
                    return Some(ToolCall {
                        tool: "skill_create".to_string(),
                        params: value,
                    });
                }

                // Heuristic for think
                if obj.contains_key("thought") && obj.len() == 1 {
                    tracing::warn!("Heuristic: detected potential think params without wrapper");
                    return Some(ToolCall {
                        tool: "think".to_string(),
                        params: value,
                    });
                }
            }
        }
    }

    None
}

fn parse_tool_call_json(input: &str) -> Option<ToolCall> {
    let value: Value = serde_json::from_str(input).ok()?;
    let obj = value.as_object()?;
    // Support both "tool" and "name" fields for tool call format
    let tool = obj
        .get("tool")
        .and_then(|v| v.as_str())
        .or_else(|| obj.get("name").and_then(|v| v.as_str()))?
        .to_string();
    let params = obj
        .get("params")
        .cloned()
        .or_else(|| obj.get("arguments").cloned())
        .unwrap_or(Value::Null);

    Some(ToolCall { tool, params })
}

fn extract_xml_tool_call(text: &str) -> Option<ToolCall> {
    // Regex for <use_tool name="...">...</use_tool>
    // Using dot matches all (?s) to handle newlines
    let tool_regex =
        Regex::new(r"(?s)<use_tool\s+name=['\x22]([^'\x22]+)['\x22]\s*>(.*?)</use_tool>").ok()?;

    if let Some(captures) = tool_regex.captures(text) {
        let tool_name = captures.get(1)?.as_str().to_string();
        let content = captures.get(2)?.as_str();

        let mut params = serde_json::Map::new();

        // Regex for <param name="...">...</param>
        // Use a loop to find all params
        let param_regex =
            Regex::new(r"(?s)<param\s+name=['\x22]([^'\x22]+)['\x22]\s*>(.*?)</param>").ok()?;

        for param_capture in param_regex.captures_iter(content) {
            if let (Some(name_match), Some(value_match)) =
                (param_capture.get(1), param_capture.get(2))
            {
                let name = name_match.as_str();
                let value = value_match.as_str().trim();

                // Try to parse as JSON if it looks like it (bool, number, null, object, array)
                let json_val = if value == "true" {
                    Value::Bool(true)
                } else if value == "false" {
                    Value::Bool(false)
                } else if let Ok(num) = value.parse::<f64>() {
                    if let Ok(int_val) = value.parse::<i64>() {
                        Value::Number(int_val.into())
                    } else {
                        if value.contains('.') {
                            serde_json::Number::from_f64(num)
                                .map(Value::Number)
                                .unwrap_or(Value::String(value.to_string()))
                        } else {
                            Value::String(value.to_string())
                        }
                    }
                } else if (value.starts_with('{') && value.ends_with('}'))
                    || (value.starts_with('[') && value.ends_with(']'))
                {
                    // Try to parse as nested JSON
                    serde_json::from_str(value).unwrap_or(Value::String(value.to_string()))
                } else {
                    Value::String(value.to_string())
                };

                params.insert(name.to_string(), json_val);
            }
        }

        return Some(ToolCall {
            tool: tool_name,
            params: Value::Object(params),
        });
    }

    None
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

/// Extract ALL JSON objects from text (not just the first one)
/// Returns them in order of appearance
fn extract_all_json_objects(text: &str) -> Vec<String> {
    let mut results = Vec::new();
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
                        results.push(text[start_idx..=i].to_string());
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }

    results
}
