//! Web tools - HTTP fetch, URL content extraction
//!
//! Provides web content fetching capabilities.

use async_trait::async_trait;
use serde_json::Value;

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// WebFetchTool - Fetch URL content
// ============================================================================

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch the content of a URL and return it as text. Supports HTML pages (converted to readable text), JSON APIs, and raw text. Use for reading documentation, API responses, or web pages."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"],
                    "description": "HTTP method (default: GET)",
                    "default": "GET"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional HTTP headers as key-value pairs"
                },
                "body": {
                    "type": "string",
                    "description": "Request body (for POST/PUT/PATCH)"
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum response length in characters (default: 50000)",
                    "default": 50000
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("url is required".into()))?;
        let method = params["method"].as_str().unwrap_or("GET");
        let headers = params["headers"].as_object();
        let body = params["body"].as_str();
        let max_length = params["max_length"].as_u64().unwrap_or(50000) as usize;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("LocaLM/0.2.0")
            .build()
            .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de créer le client HTTP: {}", e)))?;

        let mut request = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            "HEAD" => client.head(url),
            _ => return Err(ToolError::InvalidParameters(format!("Méthode HTTP inconnue: {}", method))),
        };

        // Add headers
        if let Some(hdrs) = headers {
            for (key, value) in hdrs {
                if let Some(val) = value.as_str() {
                    request = request.header(key.as_str(), val);
                }
            }
        }

        // Add body
        if let Some(b) = body {
            request = request.body(b.to_string());
        }

        let response = request
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur HTTP: {}", e)))?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let response_headers: serde_json::Map<String, Value> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|val| (k.to_string(), Value::String(val.to_string())))
            })
            .collect();

        let text = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de lire la réponse: {}", e)))?;

        // Process content based on type
        let processed = if content_type.contains("text/html") {
            html_to_text(&text)
        } else {
            text
        };

        // Truncate if needed (safe char-boundary slicing)
        let display = if processed.len() > max_length {
            let safe = crate::truncate_str(&processed, max_length);
            format!(
                "{}...\n\n[Truncated: {} chars out of {}]",
                safe,
                safe.len(),
                processed.len()
            )
        } else {
            processed
        };

        Ok(ToolResult {
            success: status < 400,
            data: serde_json::json!({
                "url": url,
                "status": status,
                "content_type": content_type,
                "content": display,
                "headers": response_headers,
                "content_length": display.len()
            }),
            message: format!("HTTP {} {} ({}, {} chars)", method, status, content_type, display.len()),
        })
    }
}

// ============================================================================
// WebDownloadTool - Download files from URL
// ============================================================================

pub struct WebDownloadTool;

#[async_trait]
impl Tool for WebDownloadTool {
    fn name(&self) -> &str {
        "web_download"
    }

    fn description(&self) -> &str {
        "Download a file from a URL and save it to disk. REQUIRES APPROVAL."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to download from"
                },
                "path": {
                    "type": "string",
                    "description": "Local file path to save to"
                }
            },
            "required": ["url", "path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("url is required".into()))?;
        let path = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("LocaLM/0.2.0")
            .build()
            .map_err(|e| ToolError::ExecutionFailed(format!("Client HTTP: {}", e)))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur HTTP: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "HTTP {} pour {}",
                response.status(),
                url
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Erreur lecture: {}", e)))?;

        let path_buf = std::path::PathBuf::from(path);
        if let Some(parent) = path_buf.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Impossible de créer le dossier: {}", e)))?;
            }
        }

        tokio::fs::write(&path_buf, &bytes)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Impossible d'écrire: {}", e)))?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "url": url,
                "path": path,
                "bytes": bytes.len()
            }),
            message: format!("Téléchargé: {} -> {} ({} octets)", url, path, bytes.len()),
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Simple HTML to text conversion (strips tags)
fn html_to_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut last_was_space = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if in_script {
            if i + 8 < lower_chars.len() && lower[i..].starts_with("</script") {
                in_script = false;
            }
            i += 1;
            continue;
        }
        if in_style {
            if i + 7 < lower_chars.len() && lower[i..].starts_with("</style") {
                in_style = false;
            }
            i += 1;
            continue;
        }

        match chars[i] {
            '<' => {
                // Check for script/style tags
                if i + 7 < lower_chars.len() && lower[i..].starts_with("<script") {
                    in_script = true;
                } else if i + 6 < lower_chars.len() && lower[i..].starts_with("<style") {
                    in_style = true;
                }
                // Block elements add newlines
                if i + 3 < chars.len() {
                    let tag_start = &lower[i..];
                    if tag_start.starts_with("<br")
                        || tag_start.starts_with("<p")
                        || tag_start.starts_with("<div")
                        || tag_start.starts_with("<h1")
                        || tag_start.starts_with("<h2")
                        || tag_start.starts_with("<h3")
                        || tag_start.starts_with("<li")
                        || tag_start.starts_with("<tr")
                    {
                        if !text.ends_with('\n') {
                            text.push('\n');
                        }
                    }
                }
                in_tag = true;
            }
            '>' => {
                in_tag = false;
            }
            '&' if !in_tag => {
                // Handle common HTML entities
                if i + 4 < chars.len() && html[i..].starts_with("&amp;") {
                    text.push('&');
                    i += 4;
                } else if i + 3 < chars.len() && html[i..].starts_with("&lt;") {
                    text.push('<');
                    i += 3;
                } else if i + 3 < chars.len() && html[i..].starts_with("&gt;") {
                    text.push('>');
                    i += 3;
                } else if i + 5 < chars.len() && html[i..].starts_with("&nbsp;") {
                    text.push(' ');
                    i += 5;
                } else if i + 5 < chars.len() && html[i..].starts_with("&quot;") {
                    text.push('"');
                    i += 5;
                } else {
                    text.push('&');
                }
            }
            c if !in_tag => {
                if c.is_whitespace() {
                    if !last_was_space {
                        text.push(' ');
                        last_was_space = true;
                    }
                } else {
                    text.push(c);
                    last_was_space = false;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Clean up: remove excessive newlines
    let mut result = String::new();
    let mut empty_lines = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            empty_lines += 1;
            if empty_lines <= 2 {
                result.push('\n');
            }
        } else {
            empty_lines = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}
