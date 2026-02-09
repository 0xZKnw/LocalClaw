# Agent Module (src/agent/)

## OVERVIEW
Core orchestration engine for LocaLM's agentic capabilities. Implements a multi-step reasoning loop, task planning, and a secure tool-calling system with 30+ built-in tools.

## STRUCTURE
- `mod.rs`: Agent configuration, registry initialization, and permission mapping.
- `loop_runner.rs`: Main execution loop state machine and event emission.
- `tools.rs`: Core `Tool` trait, registry management, and result types.
- `permissions.rs`: 6-level security system and approval workflow.
- `planning.rs`: Task decomposition and TODO management.
- `runner.rs`: Tool call extraction and LLM interaction formatting.
- `prompts.rs`: Dynamic system prompt construction.

## KEY TYPES
- `Agent`: Central coordinator holding the registry, config, and permission manager.
- `AgentLoop`: Runner instance managing the state machine for a single request.
- `ToolRegistry`: DashMap-backed thread-safe storage for all registered tools.
- `AgentContext`: Persistent state across iterations (history, plan, thinking log).
- `AgentConfig`: Boolean toggles for filesystem, web, bash, and git capabilities.

## TOOL IMPLEMENTATION
All tools must implement the `Tool` trait:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value; // JSON Schema
    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError>;
}
```
### Adding a New Tool
1. Create tool struct in `src/agent/tools/`.
2. Implement `Tool` trait.
3. Register in `Agent::initialize_tools()` within `src/agent/mod.rs`.
4. Map to appropriate `PermissionLevel` in `get_tool_permission()`.

## LOOP STATES
The `AgentLoop` follows a 9-state reasoning cycle:
1. **Analyzing**: Initial request parsing.
2. **Planning**: Creating/updating the `TaskPlan`.
3. **Thinking**: Internal reasoning and decision making.
4. **Acting**: Executing the selected tool.
5. **Observing**: Processing tool output.
6. **Reflecting**: Assessing progress against the plan.
7. **Responding**: Generating final user response.
8. **WaitingForUser**: Paused for permission or input.
9. **Completed**: Final state after success or terminal failure.

## PERMISSION LEVELS
Managed by `PermissionManager` (0 to 5):
- `ReadOnly`: `file_read`, `grep`, `git_status` (Safe)
- `WriteFile`: `file_write`, `file_edit` (Destructive)
- `ReadWrite`: Advanced file operations.
- `ExecuteSafe`: Known safe commands.
- `ExecuteUnsafe`: `bash`, `git_commit` (Full system access)
- `Network`: `web_search`, `web_fetch`, `mcp_*` (External access)

## CONVENTIONS
- **Atomic Execution**: Tools should be idempotent where possible.
- **Error Handling**: Always return `ToolError` instead of panicking.
- **Observability**: Use `tracing` for all state transitions and tool logs.
- **Isolation**: Agent logic must remain independent of specific UI components.
- **Safety**: Unsafe tools MUST be explicitly enabled in `AgentConfig`.
