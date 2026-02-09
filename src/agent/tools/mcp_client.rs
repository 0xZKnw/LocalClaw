//! Generic MCP Client - Connect to any MCP server
//!
//! Supports both stdio (child process) and HTTP/SSE transports.
//! Discovers tools from the server and creates dynamic tool wrappers.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// MCP Server Configuration
// ============================================================================

/// Configuration for an MCP server connection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Unique identifier for this server
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Transport type
    pub transport: McpTransport,
    /// Environment variables to set for the server process
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Whether this server is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpTransport {
    /// Stdio transport - spawns a child process
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// HTTP/SSE transport - connects to an HTTP server
    #[serde(rename = "http")]
    Http { url: String },
}

// ============================================================================
// MCP Tool Description (from server)
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpToolDescription {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Value,
}

// ============================================================================
// Stdio MCP Client
// ============================================================================

pub struct StdioMcpClient {
    config: McpServerConfig,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<tokio::process::ChildStdin>>,
    reader: Mutex<Option<BufReader<tokio::process::ChildStdout>>>,
    initialized: AtomicBool,
    request_id: AtomicU64,
}

impl StdioMcpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            reader: Mutex::new(None),
            initialized: AtomicBool::new(false),
            request_id: AtomicU64::new(1),
        }
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn start(&self) -> Result<(), ToolError> {
        let (command, args) = match &self.config.transport {
            McpTransport::Stdio { command, args } => (command.clone(), args.clone()),
            _ => {
                return Err(ToolError::ExecutionFailed(
                    "Not a stdio transport".into(),
                ))
            }
        };

        let mut cmd = Command::new(&command);
        cmd.args(&args);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Impossible de démarrer le serveur MCP '{}': {}. Vérifiez que '{}' est installé.",
                self.config.name, e, command
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ToolError::ExecutionFailed("Impossible d'accéder au stdin du serveur MCP".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ToolError::ExecutionFailed("Impossible d'accéder au stdout du serveur MCP".into())
        })?;

        *self.child.lock().await = Some(child);
        *self.stdin.lock().await = Some(stdin);
        *self.reader.lock().await = Some(BufReader::new(stdout));

        // Initialize MCP protocol
        self.initialize().await?;

        Ok(())
    }

    async fn initialize(&self) -> Result<(), ToolError> {
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "clientInfo": { "name": "localclaw", "version": "0.2.0" }
            }
        });

        let response = self.send_request(init_request).await?;
        tracing::info!("MCP server '{}' initialized: {:?}", self.config.name, response.get("result"));

        // Send initialized notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.send_notification(notification).await?;

        self.initialized.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn send_notification(&self, notification: Value) -> Result<(), ToolError> {
        let mut stdin = self.stdin.lock().await;
        let stdin = stdin.as_mut().ok_or_else(|| {
            ToolError::ExecutionFailed("Serveur MCP non démarré".into())
        })?;

        let msg = serde_json::to_string(&notification)
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur sérialisation: {}", e)))?;

        stdin
            .write_all(format!("{}\n", msg).as_bytes())
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur écriture stdin: {}", e)))?;
        stdin.flush().await.ok();

        Ok(())
    }

    async fn send_request(&self, request: Value) -> Result<Value, ToolError> {
        let mut stdin = self.stdin.lock().await;
        let stdin = stdin.as_mut().ok_or_else(|| {
            ToolError::ExecutionFailed("Serveur MCP non démarré".into())
        })?;

        let msg = serde_json::to_string(&request)
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur sérialisation: {}", e)))?;

        stdin
            .write_all(format!("{}\n", msg).as_bytes())
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur écriture stdin: {}", e)))?;
        stdin.flush().await.ok();
        let _ = stdin;

        // Read response
        let mut reader = self.reader.lock().await;
        let reader = reader.as_mut().ok_or_else(|| {
            ToolError::ExecutionFailed("Serveur MCP non démarré".into())
        })?;

        let mut line = String::new();
        // Read lines until we get a valid JSON-RPC response
        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Erreur lecture stdout: {}", e)))?;

            if bytes_read == 0 {
                return Err(ToolError::ExecutionFailed(
                    "Le serveur MCP a fermé la connexion".into(),
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Try to parse as JSON
            if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                // Check if it's a response (has "id" field) vs notification
                if value.get("id").is_some() {
                    return Ok(value);
                }
                // Skip notifications, keep reading
                continue;
            }
        }
    }

    /// List available tools from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<McpToolDescription>, ToolError> {
        if !self.initialized.load(Ordering::Relaxed) {
            return Err(ToolError::ExecutionFailed(
                "Serveur MCP non initialisé".into(),
            ));
        }

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "tools/list"
        });

        let response = self.send_request(request).await?;

        let tools = response
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| ToolError::ExecutionFailed("Réponse tools/list invalide".into()))?;

        let mut tool_descriptions = Vec::new();
        for tool in tools {
            if let Ok(desc) = serde_json::from_value::<McpToolDescription>(tool.clone()) {
                tool_descriptions.push(desc);
            }
        }

        Ok(tool_descriptions)
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        if !self.initialized.load(Ordering::Relaxed) {
            return Err(ToolError::ExecutionFailed(
                "Serveur MCP non initialisé".into(),
            ));
        }

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Erreur MCP inconnue");
            return Err(ToolError::ExecutionFailed(message.to_string()));
        }

        let result = response
            .get("result")
            .ok_or_else(|| ToolError::ExecutionFailed("Réponse MCP sans résultat".into()))?;

        Ok(result.clone())
    }

    pub async fn stop(&self) {
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
        }
    }
}

// ============================================================================
// HTTP MCP Client
// ============================================================================

pub struct HttpMcpClient {
    config: McpServerConfig,
    client: reqwest::Client,
    initialized: AtomicBool,
    request_id: AtomicU64,
}

impl HttpMcpClient {
    pub fn new(config: McpServerConfig) -> Self {
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

    fn url(&self) -> &str {
        match &self.config.transport {
            McpTransport::Http { url } => url,
            _ => "",
        }
    }

    pub async fn initialize(&self) -> Result<(), ToolError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "clientInfo": { "name": "localclaw", "version": "0.2.0" }
            }
        });

        self.http_request(request).await?;
        self.initialized.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn http_request(&self, request: Value) -> Result<Value, ToolError> {
        let response = self
            .client
            .post(self.url())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur HTTP MCP: {}", e)))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "HTTP MCP erreur ({}): {}",
                status, body
            )));
        }

        parse_mcp_response(&body)
    }

    pub async fn list_tools(&self) -> Result<Vec<McpToolDescription>, ToolError> {
        if !self.initialized.load(Ordering::Relaxed) {
            self.initialize().await?;
        }

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "tools/list"
        });

        let response = self.http_request(request).await?;
        let tools = response
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| ToolError::ExecutionFailed("Réponse tools/list invalide".into()))?;

        let mut descriptions = Vec::new();
        for tool in tools {
            if let Ok(desc) = serde_json::from_value::<McpToolDescription>(tool.clone()) {
                descriptions.push(desc);
            }
        }
        Ok(descriptions)
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        if !self.initialized.load(Ordering::Relaxed) {
            self.initialize().await?;
        }

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let response = self.http_request(request).await?;

        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Erreur MCP");
            return Err(ToolError::ExecutionFailed(message.to_string()));
        }

        Ok(response
            .get("result")
            .cloned()
            .unwrap_or(Value::Null))
    }
}

// ============================================================================
// Dynamic MCP Tool - Wraps any MCP server tool
// ============================================================================

/// A dynamic tool that wraps an MCP server's tool
pub struct DynamicMcpTool {
    server_id: String,
    tool_name: String,
    tool_description: String,
    input_schema: Value,
    client: Arc<dyn McpClient>,
}

/// Trait for MCP clients (both stdio and HTTP)
#[async_trait]
pub trait McpClient: Send + Sync {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, ToolError>;
}

/// Wrapper that holds an Arc<StdioMcpClient> and implements McpClient
pub struct StdioMcpClientWrapper {
    inner: Arc<StdioMcpClient>,
}

impl StdioMcpClientWrapper {
    pub fn new(client: Arc<StdioMcpClient>) -> Self {
        Self { inner: client }
    }
}

#[async_trait]
impl McpClient for StdioMcpClientWrapper {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, ToolError> {
        self.inner.call_tool(name, args).await
    }
}

/// Wrapper that holds an Arc<HttpMcpClient> and implements McpClient
pub struct HttpMcpClientWrapper {
    inner: Arc<HttpMcpClient>,
}

impl HttpMcpClientWrapper {
    pub fn new(client: Arc<HttpMcpClient>) -> Self {
        Self { inner: client }
    }
}

#[async_trait]
impl McpClient for HttpMcpClientWrapper {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, ToolError> {
        self.inner.call_tool(name, args).await
    }
}

impl DynamicMcpTool {
    pub fn new(
        server_id: String,
        desc: McpToolDescription,
        client: Arc<dyn McpClient>,
    ) -> Self {
        Self {
            server_id,
            tool_name: desc.name,
            tool_description: desc.description,
            input_schema: desc.input_schema,
            client,
        }
    }
}

#[async_trait]
impl Tool for DynamicMcpTool {
    fn name(&self) -> &str {
        // Prefix with server ID to avoid conflicts
        // This is a workaround since we can't return a String
        // The actual name will be set during registration
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters_schema(&self) -> Value {
        if self.input_schema.is_null() || self.input_schema == Value::Object(Default::default()) {
            serde_json::json!({
                "type": "object",
                "properties": {}
            })
        } else {
            self.input_schema.clone()
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        tracing::debug!(
            "MCP tool call: {}:{} with params: {:?}",
            self.server_id,
            self.tool_name,
            params
        );

        let result = self.client.call_tool(&self.tool_name, params).await?;

        // Extract text content from MCP response
        let _content_text = extract_mcp_text(&result);

        Ok(ToolResult {
            success: true,
            data: result,
            message: format!(
                "[MCP:{}] {} exécuté",
                self.server_id, self.tool_name
            ),
        })
    }
}

// ============================================================================
// MCP Server Manager - Manages multiple MCP server connections
// ============================================================================

pub struct McpServerManager {
    configs: Vec<McpServerConfig>,
    stdio_clients: HashMap<String, Arc<StdioMcpClient>>,
    http_clients: HashMap<String, Arc<HttpMcpClient>>,
}

impl McpServerManager {
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            stdio_clients: HashMap::new(),
            http_clients: HashMap::new(),
        }
    }

    /// Add a server configuration
    pub fn add_server(&mut self, config: McpServerConfig) {
        self.configs.push(config);
    }

    /// Start all configured servers and discover their tools
    pub async fn start_all(&mut self) -> Vec<Arc<dyn Tool>> {
        let mut all_tools: Vec<Arc<dyn Tool>> = Vec::new();

        for config in &self.configs {
            if !config.enabled {
                tracing::info!("MCP server '{}' is disabled, skipping", config.name);
                continue;
            }

            tracing::info!("Starting MCP server: {} ({})", config.name, config.id);

            match &config.transport {
                McpTransport::Stdio { .. } => {
                    let client = Arc::new(StdioMcpClient::new(config.clone()));
                    match client.start().await {
                        Ok(()) => {
                            match client.list_tools().await {
                                Ok(tools) => {
                                    tracing::info!(
                                        "MCP server '{}': {} tool(s) discovered",
                                        config.name,
                                        tools.len()
                                    );
                                    let client_trait: Arc<dyn McpClient> = Arc::new(StdioMcpClientWrapper::new(client.clone()));
                                    for tool_desc in tools {
                                        let prefixed_name = format!(
                                            "mcp_{}_{}", 
                                            config.id, 
                                            tool_desc.name
                                        );
                                        let dynamic_tool = DynamicMcpTool {
                                            server_id: config.id.clone(),
                                            tool_name: prefixed_name,
                                            tool_description: format!(
                                                "[MCP:{}] {}",
                                                config.name, tool_desc.description
                                            ),
                                            input_schema: tool_desc.input_schema,
                                            client: client_trait.clone(),
                                        };
                                        all_tools.push(Arc::new(dynamic_tool));
                                    }
                                    self.stdio_clients.insert(config.id.clone(), client);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to list tools from MCP server '{}': {}",
                                        config.name,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to start MCP server '{}': {}",
                                config.name,
                                e
                            );
                        }
                    }
                }
                McpTransport::Http { .. } => {
                    let client = Arc::new(HttpMcpClient::new(config.clone()));
                    match client.list_tools().await {
                        Ok(tools) => {
                            tracing::info!(
                                "MCP server '{}' (HTTP): {} tool(s) discovered",
                                config.name,
                                tools.len()
                            );
                            let client_trait: Arc<dyn McpClient> = Arc::new(HttpMcpClientWrapper::new(client.clone()));
                            for tool_desc in tools {
                                let prefixed_name = format!(
                                    "mcp_{}_{}", 
                                    config.id, 
                                    tool_desc.name
                                );
                                let dynamic_tool = DynamicMcpTool {
                                    server_id: config.id.clone(),
                                    tool_name: prefixed_name,
                                    tool_description: format!(
                                        "[MCP:{}] {}",
                                        config.name, tool_desc.description
                                    ),
                                    input_schema: tool_desc.input_schema,
                                    client: client_trait.clone(),
                                };
                                all_tools.push(Arc::new(dynamic_tool));
                            }
                            self.http_clients.insert(config.id.clone(), client);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to connect to MCP server '{}': {}",
                                config.name,
                                e
                            );
                        }
                    }
                }
            }
        }

        all_tools
    }

    /// Stop all running servers
    pub async fn stop_all(&self) {
        for (id, client) in &self.stdio_clients {
            tracing::info!("Stopping MCP server: {}", id);
            client.stop().await;
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn extract_mcp_text(result: &Value) -> String {
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
    result.to_string()
}

fn parse_mcp_response(body: &str) -> Result<Value, ToolError> {
    let trimmed = body.trim();

    // Direct JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str(trimmed)
            .map_err(|e| ToolError::ExecutionFailed(format!("Réponse MCP invalide: {}", e)));
    }

    // SSE format
    if trimmed.contains("data:") {
        for line in trimmed.lines().rev() {
            let line = line.trim();
            if line.starts_with("data:") {
                let data = line.trim_start_matches("data:").trim();
                if !data.is_empty() && data != "[DONE]" {
                    return serde_json::from_str(data).map_err(|e| {
                        ToolError::ExecutionFailed(format!("Réponse MCP SSE invalide: {}", e))
                    });
                }
            }
        }
    }

    Err(ToolError::ExecutionFailed(
        "Réponse MCP invalide: attendu JSON ou SSE".into(),
    ))
}
