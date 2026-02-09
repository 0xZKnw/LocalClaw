//! Dynamic prompt system for the agent
//!
//! Provides context injection, system reminders, and specialized prompts
//! for different agent states and tasks.

use crate::agent::loop_runner::AgentContext;
use crate::agent::planning::TaskPlan;
use crate::agent::tools::ToolInfo;

/// Build the complete system prompt with tool instructions and context
pub fn build_agent_system_prompt(
    base_prompt: &str,
    tools: &[ToolInfo],
    ctx: Option<&AgentContext>,
    plan: Option<&TaskPlan>,
) -> String {
    let mut prompt = String::new();
    
    // Base system prompt
    if !base_prompt.trim().is_empty() {
        prompt.push_str(base_prompt);
        prompt.push_str("\n\n");
    }
    
    // Agent identity and capabilities
    prompt.push_str(AGENT_IDENTITY);
    prompt.push('\n');
    
    // Thinking instructions
    prompt.push_str(THINKING_INSTRUCTIONS);
    prompt.push('\n');
    
    // Tool instructions
    if !tools.is_empty() {
        prompt.push_str(&build_tool_instructions_advanced(tools));
        prompt.push('\n');
    }
    
    // Planning instructions
    prompt.push_str(PLANNING_INSTRUCTIONS);
    prompt.push('\n');
    
    // Context injection if available
    if let Some(context) = ctx {
        prompt.push_str(&build_context_reminder(context));
        prompt.push('\n');
    }
    
    // Current plan status
    if let Some(plan) = plan {
        prompt.push_str(&build_plan_reminder(plan));
        prompt.push('\n');
    }
    
    prompt
}

/// Agent identity prompt
const AGENT_IDENTITY: &str = r#"## Identity
You are an advanced AI assistant with autonomous agent capabilities, similar to Claude Code or OpenCode. You can:
- Think and plan before acting
- Read, create, edit, delete and move files
- Execute full shell commands (bash/powershell)
- Perform Git operations (status, diff, log, commit, branch, stash)
- Search code and the web
- Fetch web pages and API content
- Compare files, do multi-file find-and-replace
- Inspect the system (processes, environment, system info)
- Connect to external MCP servers (GitHub, Brave Search, databases, etc.)
- Iterate and improve your responses

You work autonomously but ask for confirmation for dangerous actions.
You prefer editing existing files (file_edit) over full rewrites (file_write).
"#;

/// Instructions for thinking/reasoning mode
const THINKING_INSTRUCTIONS: &str = r#"## Thinking Mode
Before each important action, take time to think:

<thinking>
- What is the main objective?
- What information do I need?
- What tool is most appropriate?
- What are the potential risks?
</thinking>

You can use <thinking></thinking> tags to show your reasoning.
This content won't be shown to the user but helps you reason better.

## Error Handling
When a tool fails or an action doesn't work:
- NEVER stop after a single error
- Think in a <thinking> block about what went wrong
- Try an alternative approach (different tool, different parameters, reformulation)
- If after 2-3 attempts nothing works, explain the problem to the user and propose solutions
- You are a PERSISTENT and RESOURCEFUL assistant
"#;

/// Instructions for planning
const PLANNING_INSTRUCTIONS: &str = r#"## Planning
For complex tasks, create a structured plan:

1. Analyze the request and identify necessary steps
2. Create an ordered task list
3. Execute each task one by one
4. Verify results and adjust if necessary
5. Summarize results at the end

You can update your plan with the todo_write tool if available.
"#;

/// Build advanced tool instructions with examples
pub fn build_tool_instructions_advanced(tools: &[ToolInfo]) -> String {
    if tools.is_empty() {
        return String::new();
    }
    
    let mut out = String::from(
        r#"## Available Tools

## Tool Invocation Formats

You have two ways to invoke tools. **PREFER XML** for code generation, file editing, or complex content.

### 1. XML Format (Recommended for Code/Content)
Use this format when writing code, creating files, or sending multi-line content. It handles escaping much better.
```xml
<use_tool name="tool_name">
    <param name="param_name">Content here...</param>
    <param name="other_param">Value</param>
</use_tool>
```

### 2. JSON Format (For Simple Calls)
Use this for simple, single-line queries like searches.
```json
{"tool": "tool_name", "params": {"key": "value"}}
```

## ⚡ CONCISENESS & EXECUTION RULES
- **Executable Skills**: When creating a skill, you MUST provide an executable file (e.g., `main.py`, `run.sh`) in the `files` parameter.
- **Documentation (SKILL.md)**: The `content` parameter becomes the `SKILL.md` file. It MUST explain HOW the skill works and what the code does. It is your documentation.
- **Conciseness**: Keep reasoning under 100 words.
- **No Placeholders**: ALWAYS generate real content.


## 🚨 ABSOLUTE PROHIBITIONS - ANTI-HALLUCINATION 🚨

### NEVER DO THIS:
1. NEVER generate fake tool results (like "✅ pdf_read: ..." or "PDF Content:")
2. NEVER pretend to have executed a tool - the SYSTEM executes tools, not you
3. NEVER simulate tool output with invented text
4. NEVER say "Done" or "File created" WITHOUT receiving actual system confirmation
5. NEVER generate code blocks that look like tool results

### MANDATORY VERIFICATION:
- After requesting a tool, you MUST WAIT for the system message containing "[TOOL_RESULT]" or actual result
- IF you have NOT received a system message with the result → the tool was NOT EXECUTED
- NEVER confirm success without having SEEN the actual system result
- For file creations/writes: VERIFY with file_list or file_read afterwards to confirm

### HOW TO KNOW IF A TOOL SUCCEEDED:
1. You emit the tool JSON
2. You WAIT for system response (not your own generation!)
3. The SYSTEM responds with the REAL result (format: "[TOOL_RESULT] tool_name: ...")
4. ONLY AFTER this system response can you confirm success

### IF YOU DON'T SEE A SYSTEM RESULT:
- The tool was NOT executed
- DO NOT confirm success
- Either call the tool for real, or say you will do it

"#,
    );

    out.push_str("### Tool List:\n\n");

    for tool in tools {
        out.push_str(&format!("**{}**\n", tool.name));
        out.push_str(&format!("  Description: {}\n", tool.description));
        
        // Add schema info
        if let Some(props) = tool.parameters_schema.get("properties") {
            out.push_str("  Parameters:\n");
            if let Some(obj) = props.as_object() {
                for (name, schema) in obj {
                    let type_str = schema.get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("any");
                    let desc = schema.get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    out.push_str(&format!("    - {}: {} - {}\n", name, type_str, desc));
                }
            }
        }
        
        // Add example for common tools
        if let Some(example) = get_tool_example(&tool.name) {
            out.push_str(&format!("  Example: {}\n", example));
        }
        
        out.push('\n');
    }

    out
}

/// Get example usage for common tools
fn get_tool_example(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        // Search tools
        "web_search" => Some(r#"{"tool": "web_search", "params": {"query": "latest AI news 2024"}}"#),
        "code_search" => Some(r#"{"tool": "code_search", "params": {"query": "React hooks tutorial"}}"#),
        // File read tools
        "file_read" => Some(r#"{"tool": "file_read", "params": {"path": "src/main.rs", "start_line": 1, "end_line": 50}}"#),
        "file_list" => Some(r#"{"tool": "file_list", "params": {"path": ".", "recursive": true, "max_depth": 2}}"#),
        "file_info" => Some(r#"{"tool": "file_info", "params": {"path": "src/main.rs"}}"#),
        "file_search" => Some(r#"{"tool": "file_search", "params": {"query": "TODO", "path": "./src", "file_pattern": "rs"}}"#),
        // File write/edit tools
        "file_write" => Some(r#"<use_tool name="file_write">
    <param name="path">output.txt</param>
    <param name="content">Line 1
Line 2
Line 3</param>
</use_tool>"#),
        "file_edit" => Some(r#"<use_tool name="file_edit">
    <param name="path">src/main.rs</param>
    <param name="old_string">fn old_name()</param>
    <param name="new_string">fn new_name()</param>
</use_tool>"#),
        "file_create" => Some(r#"{"tool": "file_create", "params": {"path": "src/new_file.rs", "content": "//! New module\n"}}"#),
        "file_delete" => Some(r#"{"tool": "file_delete", "params": {"path": "temp_file.txt"}}"#),
        "file_move" => Some(r#"{"tool": "file_move", "params": {"source": "old.rs", "destination": "new.rs"}}"#),
        "file_copy" => Some(r#"{"tool": "file_copy", "params": {"source": "template.rs", "destination": "new_module.rs"}}"#),
        "directory_create" => Some(r#"{"tool": "directory_create", "params": {"path": "src/new_module"}}"#),
        // Search tools
        "grep" => Some(r#"{"tool": "grep", "params": {"pattern": "fn main", "path": "./src"}}"#),
        "glob" => Some(r#"{"tool": "glob", "params": {"pattern": "**/*.rs"}}"#),
        // Shell tools
        "bash" => Some(r#"{"tool": "bash", "params": {"command": "cargo build 2>&1", "timeout_secs": 120}}"#),
        "bash_background" => Some(r#"{"tool": "bash_background", "params": {"command": "cargo watch -x run"}}"#),
        // Git tools
        "git_status" => Some(r#"{"tool": "git_status", "params": {}}"#),
        "git_diff" => Some(r#"{"tool": "git_diff", "params": {"staged": false}}"#),
        "git_log" => Some(r#"{"tool": "git_log", "params": {"count": 10, "oneline": true}}"#),
        "git_commit" => Some(r#"{"tool": "git_commit", "params": {"message": "feat: add new feature", "files": ["src/main.rs"]}}"#),
        "git_branch" => Some(r#"{"tool": "git_branch", "params": {"action": "list"}}"#),
        "git_stash" => Some(r#"{"tool": "git_stash", "params": {"action": "save", "message": "WIP"}}"#),
        // Web tools
        "web_fetch" => Some(r#"{"tool": "web_fetch", "params": {"url": "https://api.example.com/data"}}"#),
        "web_download" => Some(r#"{"tool": "web_download", "params": {"url": "https://example.com/file.zip", "path": "downloads/file.zip"}}"#),
        // Dev tools
        "diff" => Some(r#"{"tool": "diff", "params": {"file_a": "old.rs", "file_b": "new.rs"}}"#),
        "find_replace" => Some(r#"{"tool": "find_replace", "params": {"search": "old_name", "replace": "new_name", "path": "./src", "file_pattern": "rs"}}"#),
        "patch" => Some(r#"{"tool": "patch", "params": {"path": "src/main.rs", "patch": "-old line\n+new line"}}"#),
        "wc" => Some(r#"{"tool": "wc", "params": {"path": "src/main.rs"}}"#),
        // System tools
        "tree" => Some(r#"{"tool": "tree", "params": {"path": ".", "max_depth": 3}}"#),
        "which" => Some(r#"{"tool": "which", "params": {"command": "cargo"}}"#),
        "system_info" => Some(r#"{"tool": "system_info", "params": {}}"#),
        "process_list" => Some(r#"{"tool": "process_list", "params": {"filter": "node"}}"#),
        "environment" => Some(r#"{"tool": "environment", "params": {"name": "PATH"}}"#),
        // Thinking/planning
        "think" => Some(r#"{"tool": "think", "params": {"thought": "I need to analyze the code first..."}}"#),
        "todo_write" => Some(r#"{"tool": "todo_write", "params": {"todos": [{"id": "1", "content": "Analyze the code", "status": "in_progress"}]}}"#),
        // Skill tools - simple examples
        // Skill tools
        "skill_create" => Some(r#"<use_tool name="skill_create">
    <param name="name">weather-check</param>
    <param name="description">Check weather via Python</param>
    <param name="content">This skill runs a python script to check weather. No manual steps needed.</param>
    <param name="files">{
        "main.py": "import requests\nprint(requests.get('https://wttr.in/Paris?format=3').text)"
    }</param>
</use_tool>"#),
        "skill_invoke" => Some(r#"{"tool": "skill_invoke", "params": {"name": "my-skill"}}"#),
        "skill_list" => Some(r#"{"tool": "skill_list", "params": {}}"#),
        _ => None,
    }
}

/// Build context reminder based on agent state
fn build_context_reminder(ctx: &AgentContext) -> String {
    let mut reminder = String::from("\n## Context Reminder\n");
    
    // Iteration info
    reminder.push_str(&format!(
        "- Current iteration: {}\n",
        ctx.iteration
    ));
    
    // Time elapsed
    let elapsed = ctx.elapsed().as_secs();
    if elapsed > 30 {
        reminder.push_str(&format!(
            "- Time elapsed: {}s (be mindful of time)\n",
            elapsed
        ));
    }
    
    // Recent tool usage
    if !ctx.tool_history.is_empty() {
        reminder.push_str("- Recently used tools:\n");
        for entry in ctx.tool_history.iter().rev().take(3) {
            let status = if entry.error.is_some() { "❌" } else { "✅" };
            reminder.push_str(&format!("  {} {}\n", status, entry.tool_name));
        }
    }
    
    // Warnings
    if ctx.consecutive_errors > 0 {
        reminder.push_str(&format!(
            "\n⚠️ {} consecutive error(s). Try a different approach.\n",
            ctx.consecutive_errors
        ));
    }
    
    if ctx.is_stuck() {
        reminder.push_str("\n⚠️ WARNING: You seem to be repeating the same actions. Change your approach!\n");
    }
    
    reminder
}

/// Build plan reminder
fn build_plan_reminder(plan: &TaskPlan) -> String {
    let mut reminder = String::from("\n## Current Plan\n");
    reminder.push_str(&format!("Goal: {}\n", plan.goal));
    reminder.push_str(&format!("Progress: {:.0}%\n\n", plan.progress()));
    
    // Show current and next tasks
    if let Some(current) = plan.tasks.iter().find(|t| t.status == crate::agent::planning::TaskStatus::InProgress) {
        reminder.push_str(&format!("🔄 In progress: {}\n", current.description));
    }
    
    let pending: Vec<_> = plan.pending_tasks();
    if !pending.is_empty() {
        reminder.push_str("⏳ To do:\n");
        for task in pending.iter().take(3) {
            reminder.push_str(&format!("  - {}\n", task.description));
        }
        if pending.len() > 3 {
            reminder.push_str(&format!("  ... and {} more\n", pending.len() - 3));
        }
    }
    
    reminder
}

/// Build a focused prompt for a specific task
pub fn build_task_prompt(task_description: &str, available_tools: &[&str]) -> String {
    let prompt = format!(
        r#"## Specific Task
{}

Available tools for this task: {}

Instructions:
1. Analyze the task
2. Choose the most appropriate tool
3. Execute with the correct parameters
4. Analyze the result
5. Conclude or continue if necessary
"#,
        task_description,
        available_tools.join(", ")
    );
    
    prompt
}

/// Build a reflection prompt after tool execution
pub fn build_reflection_prompt(tool_name: &str, result: &str, was_success: bool) -> String {
    if was_success {
        format!(
            r#"## Result from tool `{}`

The result is:
{}

Analyze this result and decide on the next step:
1. If you have ALL the information needed → write your complete final response to the user (no JSON, natural language)
2. If you need more data → use another tool with the correct JSON format
3. If you need to write/modify a file → use the REAL data obtained above in the file content (NEVER use placeholders)

IMPORTANT: When responding to the user, use the CONCRETE data from the result above. Don't say "here is the result" without including the actual information.
"#,
            tool_name, result
        )
    } else {
        format!(
            r#"## Tool `{}` failed

Error: {}

DO NOT STOP. Think and choose a new strategy:
1. Were the parameters correct? (check path, syntax, names)
2. Can you use another tool to achieve the same goal?
3. Can you reformulate your request?
4. If nothing works after 2 attempts, explain the problem to the user and propose alternatives.

Choose an approach and act NOW.
"#,
            tool_name, result
        )
    }
}

/// Build a summary request prompt
pub fn build_summary_prompt(context: &str) -> String {
    format!(
        r#"## Summary Request

Based on the following information:
{}

Provide a clear and concise summary that answers the user's initial question.
Include:
- Key points found
- Sources used (if relevant)
- A conclusion
"#,
        context
    )
}

/// Build a context compression prompt (OpenCode-style)
/// This asks the LLM to summarize the conversation to free up context space
pub fn build_context_compression_prompt() -> String {
    r#"## CONTEXT COMPRESSION REQUIRED

The conversation context is nearly saturated. You must now create a concise summary of everything that has happened in this conversation.

**Instructions:**
1. Summarize the ESSENTIAL points of the conversation so far
2. Include: user questions, actions performed, important results
3. Omit verbose technical details and resolved errors
4. Keep ONLY what is necessary to continue the conversation
5. Format: a dense paragraph of 200-400 words maximum

**Respond ONLY with the summary, no introduction or conclusion.**"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_build_tool_instructions() {
        let tools = vec![
            ToolInfo {
                name: "web_search".to_string(),
                description: "Search the web".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search query"}
                    }
                }),
            }
        ];
        
        let instructions = build_tool_instructions_advanced(&tools);
        assert!(instructions.contains("web_search"));
        assert!(instructions.contains("Search the web"));
    }
}
