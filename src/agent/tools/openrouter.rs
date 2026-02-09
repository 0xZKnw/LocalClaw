//! OpenRouter AI consultation tool
//!
//! Allows the local AI to consult more powerful external AI models via OpenRouter API.
//! This is useful for complex reasoning tasks where the local model needs help.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// OpenRouter Configuration
// ============================================================================

/// Default model to use (Pony Alpha from OpenRouter)
pub const DEFAULT_MODEL: &str = "openrouter/pony-alpha";

/// System prompt template for optimizing responses for local model understanding
const OPTIMIZE_SYSTEM_PROMPT: &str = r#"You are an expert assistant helping a smaller, local AI model understand complex topics.

RESPONSE FORMAT RULES:
1. Use simple, clear language - avoid jargon
2. Provide step-by-step explanations when appropriate
3. Be concise but complete
4. Use bullet points and numbered lists for clarity
5. Highlight key takeaways
6. If there's code, keep it minimal and well-commented

Your response will be relayed to a user by the local AI, so make it easy to understand and summarize."#;

// ============================================================================
// OpenRouter API types
// ============================================================================

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Option<Vec<Choice>>,
    error: Option<OpenRouterError>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterError {
    message: String,
}

// ============================================================================
// OpenRouterConsultTool - Main tool implementation
// ============================================================================

pub struct OpenRouterConsultTool;

#[async_trait]
impl Tool for OpenRouterConsultTool {
    fn name(&self) -> &str {
        "ai_consult"
    }

    fn description(&self) -> &str {
        "Consult a more powerful external AI model (via OpenRouter) for complex reasoning, explanations, or problems you struggle with. Use this when you need help understanding something complex or want a second opinion. The response will be optimized for your understanding."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question or problem to ask the powerful AI model. Be specific and provide context."
                },
                "context": {
                    "type": "string",
                    "description": "Optional additional context (e.g., relevant code, error messages, constraints)"
                },
                "model": {
                    "type": "string",
                    "description": "Model to use (default: configured in settings, or 'openrouter/pony-alpha'). Examples: 'anthropic/claude-3.5-sonnet', 'openai/gpt-4o'"
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Maximum tokens in the response (default: 1024)",
                    "default": 1024
                },
                "optimize_for_local": {
                    "type": "boolean",
                    "description": "If true, the response will be formatted for easier understanding by smaller models (default: true)",
                    "default": true
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let question = params["question"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("question is required".into()))?;
        
        let context = params["context"].as_str();
        let max_tokens = params["max_tokens"].as_u64().unwrap_or(1024) as u32;
        let optimize_for_local = params["optimize_for_local"].as_bool().unwrap_or(true);
        
        // Get model from params, or use settings, or fall back to default
        let model = params["model"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| get_configured_model());
        
        // Get API key from environment
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ToolError::ExecutionFailed(
                "OPENROUTER_API_KEY environment variable not set. Get a free key at https://openrouter.ai/keys".into()
            ))?;
        
        // Build the user message
        let user_content = if let Some(ctx) = context {
            format!("Question: {}\n\nContext:\n{}", question, ctx)
        } else {
            question.to_string()
        };
        
        // Build messages array
        let mut messages = Vec::new();
        
        if optimize_for_local {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: OPTIMIZE_SYSTEM_PROMPT.to_string(),
            });
        }
        
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_content,
        });
        
        // Create request
        let request = OpenRouterRequest {
            model: model.clone(),
            messages,
            max_tokens,
            temperature: 0.7,
        };
        
        // Make HTTP request to OpenRouter
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create HTTP client: {}", e)))?;
        
        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/localm-ai/localm")
            .header("X-Title", "LocaLM")
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;
        
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read response: {}", e)))?;
        
        if !status.is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "OpenRouter API error ({}): {}",
                status, response_text
            )));
        }
        
        // Parse response
        let api_response: OpenRouterResponse = serde_json::from_str(&response_text)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse response: {}", e)))?;
        
        // Check for API error
        if let Some(error) = api_response.error {
            return Err(ToolError::ExecutionFailed(format!(
                "OpenRouter error: {}",
                error.message
            )));
        }
        
        // Extract content from response
        let content = api_response
            .choices
            .and_then(|choices| choices.into_iter().next())
            .map(|choice| choice.message.content)
            .ok_or_else(|| ToolError::ExecutionFailed("No response content from model".into()))?;
        
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "model": model,
                "question": question,
                "response": content,
                "tokens_max": max_tokens,
                "optimized": optimize_for_local
            }),
            message: format!(
                "Réponse de {} :\n\n{}",
                model, content
            ),
        })
    }
}

/// Get the configured model from settings, or return default
fn get_configured_model() -> String {
    // Try to load from settings
    if let Ok(settings_path) = crate::storage::get_data_dir() {
        let path = settings_path.join("settings.json");
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(model) = json.get("openrouter_model").and_then(|v| v.as_str()) {
                        if !model.is_empty() {
                            return model.to_string();
                        }
                    }
                }
            }
        }
    }
    
    DEFAULT_MODEL.to_string()
}

/// Get list of popular models for UI dropdown
pub fn get_popular_models() -> Vec<(&'static str, &'static str)> {
    vec![
        ("openrouter/pony-alpha", "Pony Alpha (OpenRouter)"),
        ("nousresearch/deephermes-3-llama-3-8b-preview:free", "DeepHermes 3 8B (Free)"),
        ("google/gemma-2-9b-it:free", "Gemma 2 9B (Free)"),
        ("meta-llama/llama-3.2-3b-instruct:free", "Llama 3.2 3B (Free)"),
        ("anthropic/claude-3.5-sonnet", "Claude 3.5 Sonnet"),
        ("anthropic/claude-3-opus", "Claude 3 Opus"),
        ("openai/gpt-4o", "GPT-4o"),
        ("openai/gpt-4o-mini", "GPT-4o Mini"),
        ("google/gemini-2.0-flash-exp:free", "Gemini 2.0 Flash (Free)"),
        ("deepseek/deepseek-chat", "DeepSeek V3"),
    ]
}
