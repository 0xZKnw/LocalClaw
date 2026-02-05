//! Agent module for AI capabilities
//!
//! Provides tool calling, MCP integration, and permission management
//! for transforming LocaLM into an AI agent.

pub mod permissions;
pub mod tools;

use std::sync::Arc;

pub use permissions::{
    PermissionLevel, PermissionManager, PermissionRequest, PermissionResult,
    PermissionPolicy, PermissionSignals, PermissionDecision, PermissionNotification,
};
pub use tools::{Tool, ToolRegistry, ToolResult, ToolError};

/// Agent configuration
#[derive(Clone, Debug)]
pub struct AgentConfig {
    /// Default permission level for tools
    pub default_permission: PermissionLevel,
    /// Whether to enable tool calling
    pub enable_tools: bool,
    /// Whether to enable web search
    pub enable_web_search: bool,
    /// Whether to enable file system access
    pub enable_filesystem: bool,
    /// Whether to enable command execution
    pub enable_commands: bool,
    /// Maximum tool execution time in seconds
    pub tool_timeout_secs: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_permission: PermissionLevel::ReadOnly,
            enable_tools: true,
            enable_web_search: true,
            enable_filesystem: true,
            enable_commands: false,
            tool_timeout_secs: 30,
        }
    }
}

/// Core agent structure
pub struct Agent {
    pub config: AgentConfig,
    pub tool_registry: Arc<ToolRegistry>,
    pub permission_manager: Arc<PermissionManager>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        let tool_registry = Arc::new(ToolRegistry::new());
        let permission_manager = Arc::new(PermissionManager::new(config.default_permission));
        
        Self {
            config,
            tool_registry,
            permission_manager,
        }
    }
    
    /// Initialize default tools based on configuration
    pub async fn initialize_tools(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tools::builtins;
        
        if self.config.enable_web_search {
            // Web search tool would be registered here
            // self.tool_registry.register(Arc::new(builtins::WebSearchTool::new())).await;
        }
        
        if self.config.enable_filesystem {
            self.tool_registry.register(Arc::new(builtins::FileReadTool)).await;
            self.tool_registry.register(Arc::new(builtins::FileListTool)).await;
        }
        
        if self.config.enable_commands {
            self.tool_registry.register(Arc::new(builtins::CommandTool)).await;
        }
        
        Ok(())
    }
}
