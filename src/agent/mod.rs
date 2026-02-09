//! Agent module for AI capabilities
//!
//! Provides a sophisticated agentic system inspired by Claude Code and OpenCode:
//! - Advanced tool calling with retry and error handling
//! - Planning with TODO lists for complex multi-step tasks
//! - Thinking/reasoning mode for better decision making
//! - Dynamic prompts with context injection
//! - Multiple specialized tools (web search, code search, file operations, etc.)

pub mod permissions;
pub mod tools;
pub mod skills;
pub mod runner;
pub mod loop_runner;
pub mod planning;
pub mod prompts;
pub mod mcp_config;

use std::sync::Arc;
use skills::{SkillRegistry, loader::SkillLoader};

pub use permissions::{
    PermissionLevel, PermissionManager, PermissionRequest, PermissionResult,
    PermissionPolicy, PermissionSignals, PermissionDecision, PermissionNotification,
};
pub use tools::{Tool, ToolRegistry, ToolResult, ToolError, ToolInfo};
pub use tools::exa::{ExaSearchTool, ExaSearchConfig, create_exa_tools};
pub use tools::mcp_client::{McpServerConfig, McpTransport, McpServerManager};
pub use tools::mcp_presets::{McpPreset, McpCategory, get_all_presets};
pub use runner::{ToolCall, extract_tool_call, build_tool_instructions, format_tool_result_for_system};
pub use loop_runner::{AgentLoop, AgentLoopConfig, AgentState, AgentContext, AgentEvent, IterationResult};
pub use planning::{TaskPlan, Task, TaskStatus, TaskPriority, PlanManager};
pub use prompts::{build_agent_system_prompt, build_tool_instructions_advanced, build_context_compression_prompt};

/// Agent configuration
#[derive(Clone, Debug)]
pub struct AgentConfig {
    /// Default permission level for tools
    pub default_permission: PermissionLevel,
    /// Whether to enable tool calling
    pub enable_tools: bool,
    /// Whether to enable web search
    pub enable_web_search: bool,
    /// Whether to enable code search
    pub enable_code_search: bool,
    /// Whether to enable file system access
    pub enable_filesystem: bool,
    /// Whether to enable command execution
    pub enable_commands: bool,
    /// Whether to enable file writing/editing
    pub enable_file_write: bool,
    /// Whether to enable bash/shell execution (full access)
    pub enable_bash: bool,
    /// Whether to enable git operations
    pub enable_git: bool,
    /// Whether to enable web fetch/download
    pub enable_web_fetch: bool,
    /// Whether to enable developer tools (diff, find-replace, patch)
    pub enable_dev_tools: bool,
    /// Whether to enable system tools (process list, env, sysinfo)
    pub enable_system_tools: bool,
    /// Maximum tool execution time in seconds
    pub tool_timeout_secs: u64,
    /// Agent loop configuration
    pub loop_config: AgentLoopConfig,
    /// MCP server configurations
    pub mcp_servers: Vec<McpServerConfig>,
    /// List of disabled MCP server IDs
    pub disabled_mcp_servers: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_permission: PermissionLevel::ReadOnly,
            enable_tools: true,
            enable_web_search: true,
            enable_code_search: true,
            enable_filesystem: true,
            enable_commands: false,
            enable_file_write: true,
            enable_bash: true,
            enable_git: true,
            enable_web_fetch: true,
            enable_dev_tools: true,
            enable_system_tools: true,
            tool_timeout_secs: 120,
            loop_config: AgentLoopConfig::default(),
            mcp_servers: Vec::new(),
            disabled_mcp_servers: Vec::new(),
        }
    }
}

/// Core agent structure
pub struct Agent {
    pub config: AgentConfig,
    pub tool_registry: Arc<ToolRegistry>,
    pub permission_manager: Arc<PermissionManager>,
    pub plan_manager: PlanManager,
    pub skill_registry: Arc<SkillRegistry>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        let tool_registry = Arc::new(ToolRegistry::new());
        let permission_manager = Arc::new(PermissionManager::new(config.default_permission));
        let skill_registry = Arc::new(SkillRegistry::new());
        
        Self {
            config,
            tool_registry,
            permission_manager,
            plan_manager: PlanManager::new(),
            skill_registry,
        }
    }
    
    /// Initialize all tools based on configuration
    pub async fn initialize_tools(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tools::builtins;
        use tools::filesystem;
        use tools::shell;
        use tools::git;
        use tools::dev;
        use tools::system;
        use tools::skill_create;
        use tools::skill_invoke;
        use tools::skill_list;
        
        tracing::info!("Initializing agent tools...");
        
        // ============================================================
        // Always registered: thinking and planning tools
        // ============================================================
        self.tool_registry.register(Arc::new(builtins::ThinkTool)).await;
        self.tool_registry.register(Arc::new(builtins::TodoWriteTool)).await;
        self.tool_registry.register(Arc::new(skill_create::SkillCreateTool::new(
            self.skill_registry.clone(),
            self.tool_registry.clone(),
        ))).await;
        
        // ============================================================
        // Skill tools
        // ============================================================
        self.tool_registry.register(Arc::new(skill_invoke::SkillInvokeTool)).await;
        self.tool_registry.register(Arc::new(skill_list::SkillListTool)).await;
        tracing::info!("Core tools registered (think, todo_write, skill_create, skill_invoke, skill_list)");
        
        // ============================================================
        // Web search tools (Exa)
        // ============================================================
        if self.config.enable_web_search {
            let exa_config = ExaSearchConfig::default();
            let exa_tools = create_exa_tools(exa_config);
            for tool in exa_tools {
                self.tool_registry.register(tool).await;
            }
            tracing::info!("Exa search tools registered (web_search, code_search, company_research, deep_research, web_crawl)");
        }
        
        // ============================================================
        // File system tools (read-only)
        // ============================================================
        if self.config.enable_filesystem {
            self.tool_registry.register(Arc::new(builtins::FileReadTool)).await;
            self.tool_registry.register(Arc::new(builtins::FileListTool)).await;
            self.tool_registry.register(Arc::new(builtins::GrepTool)).await;
            self.tool_registry.register(Arc::new(builtins::GlobTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileInfoTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileSearchContentTool)).await;
            tracing::info!("Filesystem read tools registered (file_read, file_list, grep, glob, file_info, file_search)");
        }
        
        // ============================================================
        // File write/edit tools (requires permission)
        // ============================================================
        if self.config.enable_file_write {
            self.tool_registry.register(Arc::new(builtins::FileWriteTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileEditTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileCreateTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileDeleteTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileMoveTool)).await;
            self.tool_registry.register(Arc::new(filesystem::FileCopyTool)).await;
            self.tool_registry.register(Arc::new(filesystem::DirectoryCreateTool)).await;
            tracing::info!("Filesystem write tools registered (file_write, file_edit, file_create, file_delete, file_move, file_copy, directory_create)");
        }
        
        // ============================================================
        // Bash/Shell execution (full access, requires permission)
        // ============================================================
        if self.config.enable_bash {
            self.tool_registry.register(Arc::new(shell::BashTool)).await;
            self.tool_registry.register(Arc::new(shell::BashBackgroundTool)).await;
            tracing::info!("Shell tools registered (bash, bash_background)");
        }
        
        // Legacy safe command tool
        if self.config.enable_commands {
            self.tool_registry.register(Arc::new(builtins::CommandTool)).await;
            tracing::info!("Legacy command tool registered");
        }
        
        // ============================================================
        // Git tools
        // ============================================================
        if self.config.enable_git {
            self.tool_registry.register(Arc::new(git::GitStatusTool)).await;
            self.tool_registry.register(Arc::new(git::GitDiffTool)).await;
            self.tool_registry.register(Arc::new(git::GitLogTool)).await;
            self.tool_registry.register(Arc::new(git::GitCommitTool)).await;
            self.tool_registry.register(Arc::new(git::GitBranchTool)).await;
            self.tool_registry.register(Arc::new(git::GitStashTool)).await;
            tracing::info!("Git tools registered (git_status, git_diff, git_log, git_commit, git_branch, git_stash)");
        }
        
        // ============================================================
        // MCP servers (dynamic tools from external servers)
        // ============================================================
        
        // Register management tools
        self.tool_registry.register(Arc::new(tools::mcp_management::McpAddServerTool)).await;
        self.tool_registry.register(Arc::new(tools::mcp_management::McpListServersTool)).await;
        self.tool_registry.register(Arc::new(tools::mcp_management::McpRemoveServerTool)).await;
        tracing::info!("MCP management tools registered (mcp_add_server, mcp_list_servers, mcp_remove_server)");

        // Load effective config (presets + global + local)
        let mut mcp_configs = mcp_config::load_effective_config().await;
        
        // Add programmatically configured servers (overriding file configs if same ID)
        for config in &self.config.mcp_servers {
            if let Some(pos) = mcp_configs.iter().position(|c| c.id == config.id) {
                mcp_configs[pos] = config.clone();
            } else {
                mcp_configs.push(config.clone());
            }
        }

        // Filter out disabled servers
        mcp_configs.retain(|c| !self.config.disabled_mcp_servers.contains(&c.id));

        if !mcp_configs.is_empty() {
            let mut manager = McpServerManager::new();
            for server_config in mcp_configs {
                manager.add_server(server_config);
            }
            let mcp_tools = manager.start_all().await;
            let mcp_count = mcp_tools.len();
            for tool in mcp_tools {
                self.tool_registry.register(tool).await;
            }
            if mcp_count > 0 {
                tracing::info!("{} MCP tool(s) registered from external servers", mcp_count);
            }
        }
        
        // ============================================================
        // Developer tools
        // ============================================================
        if self.config.enable_dev_tools {
            self.tool_registry.register(Arc::new(dev::DiffTool)).await;
            self.tool_registry.register(Arc::new(dev::FindReplaceTool)).await;
            self.tool_registry.register(Arc::new(dev::PatchTool)).await;
            self.tool_registry.register(Arc::new(dev::CountLinesTool)).await;
            tracing::info!("Developer tools registered (diff, find_replace, patch, wc)");
        }
        
        // ============================================================
        // System tools
        // ============================================================
        if self.config.enable_system_tools {
            self.tool_registry.register(Arc::new(system::ProcessListTool)).await;
            self.tool_registry.register(Arc::new(system::EnvironmentTool)).await;
            self.tool_registry.register(Arc::new(system::SystemInfoTool)).await;
            self.tool_registry.register(Arc::new(system::WhichTool)).await;
            self.tool_registry.register(Arc::new(system::TreeTool)).await;
            tracing::info!("System tools registered (process_list, environment, system_info, which, tree)");
        }
        
        // ============================================================
        // PDF tools
        // ============================================================
        use tools::pdf;
        self.tool_registry.register(Arc::new(pdf::PdfReadTool)).await;
        self.tool_registry.register(Arc::new(pdf::PdfCreateTool)).await;
        self.tool_registry.register(Arc::new(pdf::PdfAddPageTool)).await;
        self.tool_registry.register(Arc::new(pdf::PdfMergeTool)).await;
        tracing::info!("PDF tools registered (pdf_read, pdf_create, pdf_add_page, pdf_merge)");
        
        // ============================================================
        // OpenRouter AI consultation tool
        // ============================================================
        use tools::openrouter;
        self.tool_registry.register(Arc::new(openrouter::OpenRouterConsultTool)).await;
        tracing::info!("OpenRouter tool registered (ai_consult)");
        
        // ============================================================
        // Skills (loaded from .localclaw/skills)
        // ============================================================
        tracing::info!("Loading skills...");
        let skills = SkillLoader::load_all().await;
        let skill_count = skills.len();
        for skill in skills {
            self.skill_registry.register(skill).await;
        }
        self.skill_registry.register_as_tools(&self.tool_registry).await;
        tracing::info!("{} skills loaded and registered as tools", skill_count);
        
        let total = self.tool_registry.count();
        tracing::info!("Agent initialized with {} total tools", total);
        
        Ok(())
    }
    
    /// Create an agent loop runner
    pub fn create_loop(&self) -> AgentLoop {
        AgentLoop::new(
            self.config.loop_config.clone(),
            self.tool_registry.clone(),
        )
    }
    
    /// Get list of all available tools
    pub fn list_tools(&self) -> Vec<ToolInfo> {
        self.tool_registry.list_tools()
    }
    
    /// Get system prompt with all context
    pub fn build_system_prompt(&self, base_prompt: &str) -> String {
        let tools = self.list_tools();
        let ctx = None; // Will be provided during execution
        let plan = self.plan_manager.current();
        
        build_agent_system_prompt(base_prompt, &tools, ctx, plan)
    }
}

/// Quick helper to determine permission level for a tool
pub fn get_tool_permission(tool_name: &str) -> PermissionLevel {
    match tool_name {
        // Read-only tools (no side effects)
        "file_read" | "file_list" | "grep" | "glob" | "think" | "todo_write"
        | "file_info" | "file_search" | "diff" | "wc" | "tree"
        | "process_list" | "environment" | "system_info" | "which"
        | "git_status" | "git_diff" | "git_log" | "git_branch"
        | "pdf_read"
        | "skill_list" | "skill_invoke" 
        | "mcp_list_servers" => {
            PermissionLevel::ReadOnly
        }
        // Network tools (external requests)
        "web_search" | "code_search" | "company_research" 
        | "deep_research_start" | "deep_research_check" | "web_crawl"
        | "web_fetch" | "web_download" | "ai_consult" => {
            PermissionLevel::Network
        }
        // Write tools (file modifications)
        "file_write" | "file_edit" | "file_create" | "file_delete" 
        | "file_move" | "file_copy" | "directory_create"
        | "find_replace" | "patch"
        | "pdf_create" | "pdf_add_page" | "pdf_merge"
        | "skill_create" 
        | "mcp_add_server" | "mcp_remove_server" => {
            PermissionLevel::WriteFile
        }
        // Safe command execution
        "command" => PermissionLevel::ExecuteSafe,
        // Unsafe execution (full shell, git writes)
        "bash" | "bash_background" | "git_commit" | "git_stash" => {
            PermissionLevel::ExecuteUnsafe
        }
        // MCP tools (from external servers)
        name if name.starts_with("mcp_") => PermissionLevel::Network,
        // Default to read-only
        _ => PermissionLevel::ReadOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert!(config.enable_tools);
        assert!(config.enable_web_search);
        assert!(config.enable_filesystem);
        assert!(config.enable_file_write);
        assert!(config.enable_bash);
        assert!(config.enable_git);
        assert!(config.enable_web_fetch);
        assert!(config.enable_dev_tools);
        assert!(config.enable_system_tools);
    }
    
    #[test]
    fn test_tool_permissions() {
        // Read-only
        assert_eq!(get_tool_permission("file_read"), PermissionLevel::ReadOnly);
        assert_eq!(get_tool_permission("grep"), PermissionLevel::ReadOnly);
        assert_eq!(get_tool_permission("git_status"), PermissionLevel::ReadOnly);
        assert_eq!(get_tool_permission("tree"), PermissionLevel::ReadOnly);
        assert_eq!(get_tool_permission("diff"), PermissionLevel::ReadOnly);
        // Network
        assert_eq!(get_tool_permission("web_search"), PermissionLevel::Network);
        assert_eq!(get_tool_permission("web_fetch"), PermissionLevel::Network);
        // Write
        assert_eq!(get_tool_permission("file_write"), PermissionLevel::WriteFile);
        assert_eq!(get_tool_permission("file_edit"), PermissionLevel::WriteFile);
        assert_eq!(get_tool_permission("file_create"), PermissionLevel::WriteFile);
        assert_eq!(get_tool_permission("find_replace"), PermissionLevel::WriteFile);
        // Execute
        assert_eq!(get_tool_permission("command"), PermissionLevel::ExecuteSafe);
        assert_eq!(get_tool_permission("bash"), PermissionLevel::ExecuteUnsafe);
        assert_eq!(get_tool_permission("git_commit"), PermissionLevel::ExecuteUnsafe);
        // Skill tools
        assert_eq!(get_tool_permission("skill_invoke"), PermissionLevel::ReadOnly);
        assert_eq!(get_tool_permission("skill_list"), PermissionLevel::ReadOnly);
        // MCP
        assert_eq!(get_tool_permission("mcp_github_list_repos"), PermissionLevel::Network);
    }
    
    #[tokio::test]
    async fn test_agent_initialization() {
        let config = AgentConfig {
            enable_web_search: false, // Skip network tools for test
            enable_commands: false,
            enable_web_fetch: false,
            ..Default::default()
        };
        let agent = Agent::new(config);
        agent.initialize_tools().await.unwrap();
        
        // Check all tool categories are registered
        let tools = agent.list_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        
        // Core tools
        assert!(names.contains(&"think"));
        assert!(names.contains(&"todo_write"));
        // Filesystem tools
        assert!(names.contains(&"file_read"));
        assert!(names.contains(&"grep"));
        assert!(names.contains(&"glob"));
        assert!(names.contains(&"file_info"));
        // Write tools
        assert!(names.contains(&"file_edit"));
        assert!(names.contains(&"file_create"));
        assert!(names.contains(&"file_delete"));
        // Shell tools
        assert!(names.contains(&"bash"));
        // Git tools
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_diff"));
        assert!(names.contains(&"git_log"));
        // Dev tools
        assert!(names.contains(&"diff"));
        assert!(names.contains(&"find_replace"));
        // System tools
        assert!(names.contains(&"tree"));
        assert!(names.contains(&"which"));
        assert!(names.contains(&"system_info"));
        
        // Should have 25+ tools
        assert!(tools.len() >= 25, "Expected 25+ tools, got {}", tools.len());
    }
}
