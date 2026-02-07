//! Exa search tools for web search capabilities
//!
//! Provides multiple search tools using Exa MCP:
//! - web_search: General web search
//! - code_search: Code examples and documentation search
//! - company_research: Company information lookup
//! - deep_research: In-depth research with AI analysis

use async_trait::async_trait;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::agent::tools::{Tool, ToolError, ToolResult};

/// Exa search configuration
#[derive(Clone, Debug)]
pub struct ExaSearchConfig {
    pub mcp_url: String,
    pub default_num_results: u64,
    pub default_context_chars: u64,
}

impl Default for ExaSearchConfig {
    fn default() -> Self {
        Self {
            mcp_url: std::env::var("EXA_MCP_URL")
                .or_else(|_| std::env::var("MCP_EXA_URL"))
                .unwrap_or_else(|_| "https://mcp.exa.ai/mcp".to_string()),
            default_num_results: 8,
            default_context_chars: 10000,
        }
    }
}

/// Shared MCP client for Exa
pub struct ExaMcpClient {
    config: ExaSearchConfig,
    client: reqwest::Client,
    initialized: AtomicBool,
    request_id: AtomicU64,
}

impl ExaMcpClient {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            initialized: AtomicBool::new(false),
            request_id: AtomicU64::new(1),
        }
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn mcp_request(&self, method: &str, params: Value) -> Result<Value, ToolError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": method,
            "params": params
        });

        tracing::debug!("Exa MCP request: {} - {:?}", method, params);

        let response = self
            .client
            .post(&self.config.mcp_url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("MCP request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown response".to_string());

        if !status.is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "MCP HTTP error ({}): {}",
                status, body
            )));
        }

        let value = parse_mcp_body(&body)?;

        if let Some(err) = value.get("error") {
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("MCP error");
            return Err(ToolError::ExecutionFailed(message.to_string()));
        }

        Ok(value)
    }

    pub async fn ensure_initialized(&self) -> Result<(), ToolError> {
        if self.initialized.load(Ordering::Relaxed) {
            return Ok(());
        }

        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "clientInfo": { "name": "localm", "version": "0.3.0" }
        });

        let _ = self.mcp_request("initialize", params).await?;
        self.initialized.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, ToolError> {
        self.ensure_initialized().await?;

        let result = self
            .mcp_request(
                "tools/call",
                serde_json::json!({
                    "name": tool_name,
                    "arguments": arguments
                }),
            )
            .await?;

        let tool_result = result
            .get("result")
            .ok_or_else(|| ToolError::ExecutionFailed("MCP response missing result".to_string()))?;

        let is_error = tool_result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_error {
            let error_text = extract_text(tool_result);
            return Err(ToolError::ExecutionFailed(error_text));
        }

        Ok(tool_result.clone())
    }
}

// ============================================================================
// Web Search Tool
// ============================================================================

/// General web search tool using Exa
pub struct ExaSearchTool {
    client: Arc<ExaMcpClient>,
}

impl ExaSearchTool {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            client: Arc::new(ExaMcpClient::new(config)),
        }
    }
    
    pub fn with_client(client: Arc<ExaMcpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ExaSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for any topic. Returns summarized content from top search results. Use for finding current information, news, facts, or answers to general questions."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query - be specific and include relevant keywords"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (1-10)",
                    "minimum": 1,
                    "maximum": 10,
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("query is required".to_string()))?;

        let num_results = params["num_results"]
            .as_u64()
            .map(|n| n.clamp(1, 10))
            .unwrap_or(5);

        let result = self
            .client
            .call_tool(
                "web_search_exa",
                serde_json::json!({
                    "query": query,
                    "numResults": num_results,
                    "type": "auto"
                }),
            )
            .await?;

        let content_text = extract_text(&result);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "query": query,
                "content": content_text,
                "num_results": num_results
            }),
            message: format!("Recherche web pour \"{}\" - {} résultats", query, num_results),
        })
    }
}

// ============================================================================
// Code Search Tool
// ============================================================================

/// Code-focused search tool using Exa
pub struct ExaCodeSearchTool {
    client: Arc<ExaMcpClient>,
}

impl ExaCodeSearchTool {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            client: Arc::new(ExaMcpClient::new(config)),
        }
    }
    
    pub fn with_client(client: Arc<ExaMcpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ExaCodeSearchTool {
    fn name(&self) -> &str {
        "code_search"
    }

    fn description(&self) -> &str {
        "Find code examples, documentation, and programming solutions. Searches GitHub, Stack Overflow, and official docs. Best for: API usage, library examples, code snippets, debugging help."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Programming question or code topic (e.g., 'React useState hook examples', 'Python async/await tutorial')"
                },
                "tokens_num": {
                    "type": "integer",
                    "description": "Amount of context to return (1000-50000)",
                    "minimum": 1000,
                    "maximum": 50000,
                    "default": 5000
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("query is required".to_string()))?;

        let tokens_num = params["tokens_num"]
            .as_u64()
            .map(|n| n.clamp(1000, 50000))
            .unwrap_or(5000);

        let result = self
            .client
            .call_tool(
                "get_code_context_exa",
                serde_json::json!({
                    "query": query,
                    "tokensNum": tokens_num
                }),
            )
            .await?;

        let content_text = extract_text(&result);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "query": query,
                "content": content_text,
                "tokens": tokens_num
            }),
            message: format!("Recherche code pour \"{}\"", query),
        })
    }
}

// ============================================================================
// Company Research Tool
// ============================================================================

/// Company research tool using Exa
pub struct ExaCompanyResearchTool {
    client: Arc<ExaMcpClient>,
}

impl ExaCompanyResearchTool {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            client: Arc::new(ExaMcpClient::new(config)),
        }
    }
    
    pub fn with_client(client: Arc<ExaMcpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ExaCompanyResearchTool {
    fn name(&self) -> &str {
        "company_research"
    }

    fn description(&self) -> &str {
        "Research any company to get business information, news, and insights. Returns: company products/services, recent news, industry position."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "company_name": {
                    "type": "string",
                    "description": "Name of the company to research"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of sources to check",
                    "default": 3
                }
            },
            "required": ["company_name"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let company_name = params["company_name"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("company_name is required".to_string()))?;

        let num_results = params["num_results"].as_u64().unwrap_or(3);

        let result = self
            .client
            .call_tool(
                "company_research_exa",
                serde_json::json!({
                    "companyName": company_name,
                    "numResults": num_results
                }),
            )
            .await?;

        let content_text = extract_text(&result);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "company": company_name,
                "content": content_text
            }),
            message: format!("Recherche entreprise: {}", company_name),
        })
    }
}

// ============================================================================
// Deep Research Tool (Start)
// ============================================================================

/// Start deep research using Exa's AI researcher
pub struct ExaDeepResearchStartTool {
    client: Arc<ExaMcpClient>,
}

impl ExaDeepResearchStartTool {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            client: Arc::new(ExaMcpClient::new(config)),
        }
    }
    
    pub fn with_client(client: Arc<ExaMcpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ExaDeepResearchStartTool {
    fn name(&self) -> &str {
        "deep_research_start"
    }

    fn description(&self) -> &str {
        "Start an AI-powered deep research task. The AI will search the web, read many sources, and think deeply about your question. Returns a task_id to check results later with deep_research_check."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Complex research question requiring in-depth analysis"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("query is required".to_string()))?;

        let result = self
            .client
            .call_tool(
                "deep_researcher_start",
                serde_json::json!({
                    "query": query
                }),
            )
            .await?;

        let task_id = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        
        // Try to extract task_id from response
        let extracted_id = if task_id.contains("task_id") || task_id.contains("ID") {
            task_id.to_string()
        } else {
            extract_text(&result)
        };

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "query": query,
                "task_info": extracted_id,
                "status": "started"
            }),
            message: format!("Recherche approfondie démarrée pour: {}", query),
        })
    }
}

// ============================================================================
// Deep Research Tool (Check)
// ============================================================================

/// Check deep research results using Exa
pub struct ExaDeepResearchCheckTool {
    client: Arc<ExaMcpClient>,
}

impl ExaDeepResearchCheckTool {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            client: Arc::new(ExaMcpClient::new(config)),
        }
    }
    
    pub fn with_client(client: Arc<ExaMcpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ExaDeepResearchCheckTool {
    fn name(&self) -> &str {
        "deep_research_check"
    }

    fn description(&self) -> &str {
        "Check the status and get results from a deep research task started with deep_research_start."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID returned by deep_research_start"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let task_id = params["task_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("task_id is required".to_string()))?;

        let result = self
            .client
            .call_tool(
                "deep_researcher_check",
                serde_json::json!({
                    "taskId": task_id
                }),
            )
            .await?;

        let content_text = extract_text(&result);
        
        // Determine status based on content
        let status = if content_text.contains("pending") || content_text.contains("running") {
            "in_progress"
        } else if content_text.contains("error") || content_text.contains("failed") {
            "failed"
        } else {
            "completed"
        };

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "task_id": task_id,
                "status": status,
                "content": content_text
            }),
            message: format!("Statut recherche: {}", status),
        })
    }
}

// ============================================================================
// Web Crawling Tool
// ============================================================================

/// Crawl specific URL content using Exa
pub struct ExaCrawlTool {
    client: Arc<ExaMcpClient>,
}

impl ExaCrawlTool {
    pub fn new(config: ExaSearchConfig) -> Self {
        Self {
            client: Arc::new(ExaMcpClient::new(config)),
        }
    }
    
    pub fn with_client(client: Arc<ExaMcpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for ExaCrawlTool {
    fn name(&self) -> &str {
        "web_crawl"
    }

    fn description(&self) -> &str {
        "Get the full content of a specific webpage from a known URL. Use when you have an exact URL and need its content."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to crawl"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("url is required".to_string()))?;

        let result = self
            .client
            .call_tool(
                "crawling_exa",
                serde_json::json!({
                    "url": url
                }),
            )
            .await?;

        let content_text = extract_text(&result);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "url": url,
                "content": content_text
            }),
            message: format!("Contenu extrait de: {}", url),
        })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn extract_text(result: &Value) -> String {
    // Try to get content array
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        let mut out = String::new();
        for item in content {
            if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(text);
                }
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    
    // Try direct text field
    if let Some(text) = result.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    
    // Fallback to JSON string
    result.to_string()
}

fn parse_mcp_body(body: &str) -> Result<Value, ToolError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(ToolError::ExecutionFailed(
            "Invalid MCP response: empty body".to_string(),
        ));
    }

    // Direct JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str(trimmed)
            .map_err(|e| ToolError::ExecutionFailed(format!("Invalid MCP response: {}", e)));
    }

    // SSE format
    if trimmed.contains("data:") {
        if let Some(last_json) = extract_last_sse_json(trimmed) {
            return serde_json::from_str(&last_json)
                .map_err(|e| ToolError::ExecutionFailed(format!("Invalid MCP response: {}", e)));
        }
    }

    Err(ToolError::ExecutionFailed(
        "Invalid MCP response: expected JSON or SSE data".to_string(),
    ))
}

fn extract_last_sse_json(body: &str) -> Option<String> {
    let mut last_json: Option<String> = None;
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with("data:") {
            continue;
        }
        let data = line.trim_start_matches("data:").trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        last_json = Some(data.to_string());
    }
    last_json
}

// ============================================================================
// Factory function to create all Exa tools
// ============================================================================

/// Create all Exa tools with a shared client
pub fn create_exa_tools(config: ExaSearchConfig) -> Vec<Arc<dyn Tool>> {
    let client = Arc::new(ExaMcpClient::new(config));
    
    vec![
        Arc::new(ExaSearchTool::with_client(client.clone())) as Arc<dyn Tool>,
        Arc::new(ExaCodeSearchTool::with_client(client.clone())) as Arc<dyn Tool>,
        Arc::new(ExaCompanyResearchTool::with_client(client.clone())) as Arc<dyn Tool>,
        Arc::new(ExaDeepResearchStartTool::with_client(client.clone())) as Arc<dyn Tool>,
        Arc::new(ExaDeepResearchCheckTool::with_client(client.clone())) as Arc<dyn Tool>,
        Arc::new(ExaCrawlTool::with_client(client)) as Arc<dyn Tool>,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exa_tool_names() {
        let config = ExaSearchConfig::default();
        let tools = create_exa_tools(config);
        
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"code_search"));
        assert!(names.contains(&"company_research"));
    }
    
    #[test]
    fn test_extract_text() {
        let result = serde_json::json!({
            "content": [
                {"type": "text", "text": "First result"},
                {"type": "text", "text": "Second result"}
            ]
        });
        
        let text = extract_text(&result);
        assert!(text.contains("First result"));
        assert!(text.contains("Second result"));
    }
}
