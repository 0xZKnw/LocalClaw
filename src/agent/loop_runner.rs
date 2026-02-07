//! Advanced Agent Loop System
//!
//! Implements a sophisticated agentic loop inspired by Claude Code and OpenCode.
//! Features:
//! - State machine (Thinking → Acting → Observing → Reflecting)
//! - Automatic retry with exponential backoff
//! - Infinite loop detection
//! - Dynamic planning with TODO lists
//! - Configurable iteration limits

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::agent::tools::{ToolRegistry, ToolResult, ToolError};
use crate::agent::planning::{TaskPlan, TaskStatus, PlanManager};
use crate::agent::runner::{ToolCall, extract_tool_call};

/// Agent loop configuration
#[derive(Clone, Debug)]
pub struct AgentLoopConfig {
    /// Maximum iterations per request
    pub max_iterations: usize,
    /// Maximum consecutive errors before giving up
    pub max_consecutive_errors: usize,
    /// Maximum time for entire agent run (seconds)
    pub max_runtime_secs: u64,
    /// Enable thinking/reasoning mode
    pub enable_thinking: bool,
    /// Enable automatic planning
    pub enable_planning: bool,
    /// Minimum delay between iterations (ms)
    pub min_iteration_delay_ms: u64,
    /// Enable retry on tool errors
    pub enable_retry: bool,
    /// Maximum retries per tool call
    pub max_retries: usize,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 25,
            max_consecutive_errors: 3,
            max_runtime_secs: 300, // 5 minutes
            enable_thinking: true,
            enable_planning: true,
            min_iteration_delay_ms: 100,
            enable_retry: true,
            max_retries: 2,
        }
    }
}

/// Current state of the agent loop
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// Initial state - analyzing the request
    Analyzing,
    /// Creating or updating the plan
    Planning,
    /// Thinking/reasoning about the next step
    Thinking,
    /// Executing a tool
    Acting,
    /// Processing tool results
    Observing,
    /// Reflecting on progress and deciding next action
    Reflecting,
    /// Generating final response
    Responding,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed(String),
    /// Waiting for user input/permission
    WaitingForUser,
}

/// Event emitted during agent execution
#[derive(Clone, Debug, Serialize)]
pub enum AgentEvent {
    /// State changed
    StateChanged { from: AgentState, to: AgentState },
    /// Thinking/reasoning output
    Thinking { content: String },
    /// Tool call initiated
    ToolCallStarted { tool: String, params: Value },
    /// Tool call completed
    ToolCallCompleted { tool: String, result: ToolResult },
    /// Tool call failed
    ToolCallFailed { tool: String, error: String, retry_count: usize },
    /// Plan updated
    PlanUpdated { plan: TaskPlan },
    /// Progress update
    Progress { iteration: usize, max_iterations: usize, message: String },
    /// Partial response text
    ResponseChunk { text: String },
    /// Agent completed
    Completed { final_response: String },
    /// Agent failed
    Failed { error: String },
}

/// Result of a single iteration
#[derive(Debug)]
pub enum IterationResult {
    /// Continue to next iteration
    Continue,
    /// Need to call a tool
    ToolCall(ToolCall),
    /// Final response ready
    Complete(String),
    /// Error occurred
    Error(String),
    /// Waiting for external input
    WaitForInput,
}

/// Context maintained across iterations
#[derive(Clone, Debug)]
pub struct AgentContext {
    /// Unique run ID
    pub run_id: Uuid,
    /// Current state
    pub state: AgentState,
    /// Current iteration
    pub iteration: usize,
    /// Consecutive errors count
    pub consecutive_errors: usize,
    /// Start time
    pub start_time: Instant,
    /// Current plan (if planning enabled)
    pub plan: Option<TaskPlan>,
    /// History of tool calls and results
    pub tool_history: Vec<ToolHistoryEntry>,
    /// Accumulated thinking/reasoning
    pub thinking_log: Vec<String>,
    /// Last LLM response
    pub last_response: Option<String>,
    /// Detected patterns (for loop detection)
    pub detected_patterns: Vec<String>,
}

impl AgentContext {
    pub fn new() -> Self {
        Self {
            run_id: Uuid::new_v4(),
            state: AgentState::Analyzing,
            iteration: 0,
            consecutive_errors: 0,
            start_time: Instant::now(),
            plan: None,
            tool_history: Vec::new(),
            thinking_log: Vec::new(),
            last_response: None,
            detected_patterns: Vec::new(),
        }
    }
    
    /// Check if we're stuck in a loop (repeated tool calls with same params)
    pub fn is_stuck(&self) -> bool {
        if self.tool_history.len() < 3 {
            return false;
        }
        
        // Check last 3 tool calls for repetition
        let last_three: Vec<_> = self.tool_history.iter().rev().take(3).collect();
        if last_three.len() < 3 {
            return false;
        }
        
        // Check if all three have same tool and similar params
        let first = &last_three[0];
        last_three.iter().all(|entry| {
            entry.tool_name == first.tool_name && 
            entry.params.to_string() == first.params.to_string()
        })
    }
    
    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Entry in tool call history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolHistoryEntry {
    pub tool_name: String,
    pub params: Value,
    pub result: Option<ToolResult>,
    pub error: Option<String>,
    pub timestamp: u64,
    pub duration_ms: u64,
}

/// The main agent loop runner
pub struct AgentLoop {
    pub config: AgentLoopConfig,
    pub tool_registry: Arc<ToolRegistry>,
    pub plan_manager: PlanManager,
}

impl AgentLoop {
    pub fn new(config: AgentLoopConfig, tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            config,
            tool_registry,
            plan_manager: PlanManager::new(),
        }
    }
    
    /// Analyze LLM response and determine next action
    pub fn analyze_response(&self, response: &str, ctx: &AgentContext) -> IterationResult {
        let trimmed = response.trim();
        
        // Check for empty response
        if trimmed.is_empty() {
            return IterationResult::Error("Empty response from LLM".to_string());
        }
        
        // Try to extract a tool call
        if let Some(tool_call) = extract_tool_call(response) {
            // Validate tool exists
            if self.tool_registry.get(&tool_call.tool).is_some() {
                return IterationResult::ToolCall(tool_call);
            } else {
                // Unknown tool - might be a hallucination, continue
                tracing::warn!("Unknown tool requested: {}", tool_call.tool);
            }
        }
        
        // Check for planning markers
        if self.config.enable_planning && contains_plan_markers(response) {
            // Response contains a plan that should be extracted
            return IterationResult::Continue;
        }
        
        // Check for thinking markers
        if self.config.enable_thinking && contains_thinking_markers(response) {
            // Response is thinking/reasoning, continue
            return IterationResult::Continue;
        }
        
        // Check for completion indicators
        if is_final_response(response, ctx) {
            return IterationResult::Complete(response.to_string());
        }
        
        // Default: treat as ongoing response, continue
        IterationResult::Continue
    }
    
    /// Check if we should stop the loop
    pub fn should_stop(&self, ctx: &AgentContext) -> Option<String> {
        // Check iteration limit
        if ctx.iteration >= self.config.max_iterations {
            return Some(format!(
                "Limite d'itérations atteinte ({}/{})",
                ctx.iteration, self.config.max_iterations
            ));
        }
        
        // Check consecutive errors
        if ctx.consecutive_errors >= self.config.max_consecutive_errors {
            return Some(format!(
                "Trop d'erreurs consécutives ({}/{})",
                ctx.consecutive_errors, self.config.max_consecutive_errors
            ));
        }
        
        // Check runtime
        let elapsed = ctx.elapsed().as_secs();
        if elapsed >= self.config.max_runtime_secs {
            return Some(format!(
                "Temps d'exécution maximal atteint ({:.0}s/{:.0}s)",
                elapsed, self.config.max_runtime_secs
            ));
        }
        
        // Check for stuck loop
        if ctx.is_stuck() {
            return Some("Boucle détectée - l'agent répète les mêmes actions".to_string());
        }
        
        None
    }
    
    /// Execute a tool call with retry logic
    pub async fn execute_tool_with_retry(
        &self,
        tool_call: &ToolCall,
        ctx: &mut AgentContext,
        event_tx: &mpsc::Sender<AgentEvent>,
    ) -> Result<ToolResult, ToolError> {
        let tool = self.tool_registry.get(&tool_call.tool)
            .ok_or_else(|| ToolError::NotFound(tool_call.tool.clone()))?;
        
        let mut retry_count = 0;
        let max_retries = if self.config.enable_retry { self.config.max_retries } else { 0 };
        
        loop {
            let start = Instant::now();
            
            let _ = event_tx.send(AgentEvent::ToolCallStarted {
                tool: tool_call.tool.clone(),
                params: tool_call.params.clone(),
            }).await;
            
            match tool.execute(tool_call.params.clone()).await {
                Ok(result) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    
                    // Record in history
                    ctx.tool_history.push(ToolHistoryEntry {
                        tool_name: tool_call.tool.clone(),
                        params: tool_call.params.clone(),
                        result: Some(result.clone()),
                        error: None,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                        duration_ms,
                    });
                    
                    let _ = event_tx.send(AgentEvent::ToolCallCompleted {
                        tool: tool_call.tool.clone(),
                        result: result.clone(),
                    }).await;
                    
                    return Ok(result);
                }
                Err(e) => {
                    retry_count += 1;
                    
                    let _ = event_tx.send(AgentEvent::ToolCallFailed {
                        tool: tool_call.tool.clone(),
                        error: e.to_string(),
                        retry_count,
                    }).await;
                    
                    if retry_count > max_retries {
                        // Record failure in history
                        ctx.tool_history.push(ToolHistoryEntry {
                            tool_name: tool_call.tool.clone(),
                            params: tool_call.params.clone(),
                            result: None,
                            error: Some(e.to_string()),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                            duration_ms: start.elapsed().as_millis() as u64,
                        });
                        
                        return Err(e);
                    }
                    
                    // Exponential backoff
                    let delay = Duration::from_millis(100 * (2_u64.pow(retry_count as u32 - 1)));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    
    /// Build context summary for system prompt injection
    pub fn build_context_summary(&self, ctx: &AgentContext) -> String {
        let mut summary = String::new();
        
        // Add iteration info
        summary.push_str(&format!(
            "\n## État de l'agent\n- Itération: {}/{}\n- Temps écoulé: {:.1}s\n",
            ctx.iteration,
            self.config.max_iterations,
            ctx.elapsed().as_secs_f64()
        ));
        
        // Add plan summary if exists
        if let Some(ref plan) = ctx.plan {
            summary.push_str("\n## Plan actuel\n");
            for task in &plan.tasks {
                let status_icon = match task.status {
                    TaskStatus::Pending => "⏳",
                    TaskStatus::InProgress => "🔄",
                    TaskStatus::Completed => "✅",
                    TaskStatus::Failed => "❌",
                    TaskStatus::Skipped => "⏭️",
                };
                summary.push_str(&format!("{} {}\n", status_icon, task.description));
            }
        }
        
        // Add recent tool history
        if !ctx.tool_history.is_empty() {
            summary.push_str("\n## Outils récemment utilisés\n");
            for entry in ctx.tool_history.iter().rev().take(5) {
                let status = if entry.error.is_some() { "❌" } else { "✅" };
                summary.push_str(&format!("{} {} ({}ms)\n", status, entry.tool_name, entry.duration_ms));
            }
        }
        
        summary
    }
}

/// Check if response contains plan markers
fn contains_plan_markers(response: &str) -> bool {
    let markers = [
        "\"plan\":", "\"tasks\":", "\"todo\":",
        "## Plan", "## Étapes", "## Tasks",
        "1.", "- [ ]", "- [x]",
    ];
    markers.iter().any(|m| response.contains(m))
}

/// Check if response contains thinking markers
fn contains_thinking_markers(response: &str) -> bool {
    let markers = [
        "<thinking>", "</thinking>",
        "<réflexion>", "</réflexion>",
        "Je réfléchis", "Analysons",
        "Let me think", "I need to",
    ];
    markers.iter().any(|m| response.to_lowercase().contains(&m.to_lowercase()))
}

/// Check if response is a final answer (not requiring more tool calls)
fn is_final_response(response: &str, ctx: &AgentContext) -> bool {
    // If no tool call was extracted and we have some history, likely final
    if ctx.tool_history.is_empty() {
        return false; // First response, probably needs tools
    }
    
    // Check for final answer indicators
    let final_indicators = [
        "En résumé", "En conclusion", "Pour conclure",
        "Voici la réponse", "J'ai terminé",
        "In summary", "In conclusion", "To summarize",
        "Here's the answer", "I've completed",
    ];
    
    final_indicators.iter().any(|ind| response.contains(ind))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_agent_context_new() {
        let ctx = AgentContext::new();
        assert_eq!(ctx.state, AgentState::Analyzing);
        assert_eq!(ctx.iteration, 0);
        assert!(ctx.tool_history.is_empty());
    }
    
    #[test]
    fn test_should_stop_max_iterations() {
        let config = AgentLoopConfig {
            max_iterations: 5,
            ..Default::default()
        };
        let loop_runner = AgentLoop::new(config, Arc::new(ToolRegistry::new()));
        
        let mut ctx = AgentContext::new();
        ctx.iteration = 5;
        
        assert!(loop_runner.should_stop(&ctx).is_some());
    }
    
    #[test]
    fn test_stuck_detection() {
        let mut ctx = AgentContext::new();
        
        // Add 3 identical tool calls
        for _ in 0..3 {
            ctx.tool_history.push(ToolHistoryEntry {
                tool_name: "web_search".to_string(),
                params: serde_json::json!({"query": "test"}),
                result: None,
                error: None,
                timestamp: 0,
                duration_ms: 100,
            });
        }
        
        assert!(ctx.is_stuck());
    }
}
