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

/// Progress state for loop detection
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressState {
    /// Agent is making progress on the task
    MakingProgress,
    /// Agent is stuck in a loop
    Stuck,
    /// Agent is regressing (failures increasing)
    Regressing,
    /// Unknown progress state
    Unknown,
}

/// Reason for anchoring a message - determines preservation priority
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnchorReason {
    /// Initial user goal/request (highest priority - never dropped)
    Goal,
    /// Important decision made by the agent
    Decision,
    /// Error that was successfully fixed
    ErrorFixed,
    /// Successful tool execution with meaningful result
    Success,
    /// Important tool result to preserve
    ToolResult,
}

/// Anchor message - critical information preserved during context compression
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnchorMessage {
    /// The anchored content
    pub content: String,
    /// Reason for anchoring
    pub reason: AnchorReason,
    /// When this was anchored (elapsed nanoseconds from start)
    pub timestamp: u64,
    /// Iteration when this was anchored
    pub iteration: usize,
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
    /// Number of successful tool executions
    pub success_count: usize,
    /// Number of failed tool executions
    pub failure_count: usize,
    /// Approaches attempted (for loop detection)
    pub attempted_approaches: Vec<String>,
    /// Number of iterations stuck in a loop
    pub stuck_iterations: usize,
    /// Current progress state
    pub progress_state: ProgressState,
    /// Anchor messages - critical info preserved during compression
    pub anchor_messages: Vec<AnchorMessage>,
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
            success_count: 0,
            failure_count: 0,
            attempted_approaches: Vec::new(),
            stuck_iterations: 0,
            progress_state: ProgressState::Unknown,
            anchor_messages: Vec::new(),
        }
    }
    
    /// Check if we're stuck in a loop (repeated tool calls, text patterns, or no progress)
    pub fn is_stuck(&self) -> bool {
        // Check last 3 tool calls for repetition
        if self.tool_history.len() >= 3 {
            let last_three: Vec<_> = self.tool_history.iter().rev().take(3).collect();
            let first = &last_three[0];
            if last_three.iter().all(|entry| {
                entry.tool_name == first.tool_name && 
                entry.params.to_string() == first.params.to_string()
            }) {
                tracing::warn!("Stuck: repeated tool calls detected");
                return true;
            }
        }
        
        // Check for repeated response patterns (text loop without tools)
        if let Some(ref last) = self.last_response {
            // Check if any detected pattern appears in current response
            for pattern in &self.detected_patterns {
                if pattern.len() > 30 && last.contains(pattern) {
                    tracing::warn!("Stuck: repeated text pattern detected");
                    return true;
                }
            }
        }
        
        // Check for too many iterations without any tool calls
        if self.iteration > 4 && self.tool_history.is_empty() {
            tracing::warn!("Stuck: {} iterations without tool usage", self.iteration);
            return true;
        }
        
        // Check for excessive iterations relative to tool calls
        if self.iteration > 8 && self.tool_history.len() < 2 {
            tracing::warn!("Stuck: {} iterations with only {} tool calls", 
                self.iteration, self.tool_history.len());
            return true;
        }
        
        // NEW: Check for repeated approach strings in tool results
        if self.attempted_approaches.len() >= 3 {
            let last_three_approaches: Vec<_> = self.attempted_approaches.iter().rev().take(3).collect();
            if last_three_approaches.windows(2).all(|w| w[0] == w[1]) {
                tracing::warn!("Stuck: repeated approach strings detected: {}", last_three_approaches[0]);
                return true;
            }
        }
        
        // NEW: Check for progress regression (failures increasing, successes decreasing)
        if self.iteration >= 5 && self.failure_count > 0 {
            let total_attempts = self.success_count + self.failure_count;
            if total_attempts >= 3 {
                let failure_ratio = self.failure_count as f64 / total_attempts as f64;
                // If more than 60% failures, consider it regressing
                if failure_ratio > 0.6 {
                    tracing::warn!("Stuck: progress regression detected ({} failures, {} successes)", 
                        self.failure_count, self.success_count);
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Record a response pattern for loop detection
    pub fn record_response(&mut self, response: &str) {
        self.last_response = Some(response.to_string());
        
        // Extract significant patterns (chunks that might repeat)
        if response.len() > 100 {
            let pattern = response.chars().skip(50).take(80).collect::<String>();
            if !pattern.trim().is_empty() {
                self.detected_patterns.push(pattern);
                // Keep only last 5 patterns
                if self.detected_patterns.len() > 5 {
                    self.detected_patterns.remove(0);
                }
            }
        }
    }
    
    /// Record an approach that was attempted (for loop detection)
    pub fn record_approach(&mut self, approach: &str) {
        let approach_str = approach.to_string();
        // Keep only last 10 approaches to avoid unbounded growth
        if self.attempted_approaches.len() >= 10 {
            self.attempted_approaches.remove(0);
        }
        self.attempted_approaches.push(approach_str);
    }
    
    /// Record a successful tool execution
    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.consecutive_errors = 0;
        self.update_progress_state();
    }
    
    /// Record a failed tool execution
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.consecutive_errors += 1;
        self.update_progress_state();
    }
    
    /// Update progress state based on success/failure counts
    fn update_progress_state(&mut self) {
        if self.iteration < 2 {
            self.progress_state = ProgressState::Unknown;
            return;
        }
        
        let total = self.success_count + self.failure_count;
        if total == 0 {
            self.progress_state = ProgressState::Unknown;
            return;
        }
        
        let failure_ratio = self.failure_count as f64 / total as f64;
        
        if failure_ratio > 0.6 {
            self.progress_state = ProgressState::Regressing;
        } else if failure_ratio < 0.4 {
            self.progress_state = ProgressState::MakingProgress;
        } else {
            // Check if we're stuck in a loop
            if self.is_stuck() {
                self.progress_state = ProgressState::Stuck;
            } else {
                self.progress_state = ProgressState::Unknown;
            }
        }
    }
    
    /// Check if we should force a summary response (stuck for 2+ iterations)
    pub fn should_force_summarize(&self) -> bool {
        // Check if stuck for 2 or more consecutive iterations
        if self.stuck_iterations >= 2 {
            tracing::warn!("Forcing summary after {} stuck iterations", self.stuck_iterations);
            return true;
        }
        
        // Also check for severe regression
        if self.progress_state == ProgressState::Regressing && self.iteration >= 5 {
            let total = self.success_count + self.failure_count;
            if total >= 3 && self.failure_count >= 3 {
                tracing::warn!("Forcing summary due to severe regression");
                return true;
            }
        }
        
        false
    }
    
    /// Update stuck iteration counter (call this each iteration)
    pub fn update_stuck_counter(&mut self) {
        if self.is_stuck() {
            self.stuck_iterations += 1;
        } else {
            // Reset stuck counter if making progress
            if self.progress_state == ProgressState::MakingProgress {
                self.stuck_iterations = 0;
            }
        }
    }
    
    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Add an anchor message for preservation during context compression
    /// - First user message → ALWAYS anchor as Goal
    /// - When tool succeeds → anchor as Success if meaningful result
    /// - When error is fixed → anchor as ErrorFixed
    /// - When agent makes decision → anchor as Decision
    pub fn add_anchor(&mut self, content: String, reason: AnchorReason) {
        let anchor = AnchorMessage {
            content,
            reason: reason.clone(),
            timestamp: self.elapsed().as_nanos() as u64,
            iteration: self.iteration,
        };
        
        tracing::debug!("Adding anchor message: {:?}", reason);
        
        // Goal anchors are never dropped
        if reason == AnchorReason::Goal {
            self.anchor_messages.retain(|a| a.reason != AnchorReason::Goal);
            self.anchor_messages.insert(0, anchor);
            return;
        }
        
        // Add the new anchor
        self.anchor_messages.push(anchor);
        
        // Keep max 5 anchors, dropping oldest non-Goal anchors first
        while self.anchor_messages.len() > 5 {
            // Find the oldest non-Goal anchor to drop
            if let Some(idx) = self.anchor_messages.iter()
                .position(|a| a.reason != AnchorReason::Goal) 
            {
                self.anchor_messages.remove(idx);
            } else {
                // All anchors are Goal type, drop the oldest
                self.anchor_messages.remove(0);
                break;
            }
        }
    }
    
    /// Get all anchors for preservation during context compression
    pub fn get_anchors(&self) -> Vec<AnchorMessage> {
        self.anchor_messages.clone()
    }
    
    /// Check if a specific anchor should be preserved (not compressed)
    /// Goal anchors are NEVER dropped
    pub fn should_compress_anchor(&self, anchor: &AnchorMessage) -> bool {
        // Goal anchors are never compressed
        if anchor.reason == AnchorReason::Goal {
            return false;
        }
        
        // Check if we're at max capacity and this is an older non-Goal anchor
        if self.anchor_messages.len() >= 5 {
            // Keep recent anchors (within last 2 iterations)
            let recent_threshold = self.iteration.saturating_sub(2);
            if anchor.iteration < recent_threshold {
                return true; // Should be compressed/dropped
            }
        }
        
        false
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
                // Unknown tool - return error instead of silently continuing
                tracing::error!("Unknown tool requested: {} - this tool is not available", tool_call.tool);
                return IterationResult::Error(format!(
                    "Outil '{}' non disponible ou inconnu. Les outils disponibles sont: web_search, file_read, grep, etc.",
                    tool_call.tool
                ));
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
    
    // Short responses after tool usage are usually final
    if response.len() < 500 && !response.contains("{\"tool\"") {
        return true;
    }
    
    // Check for final answer indicators
    let final_indicators = [
        "En résumé", "En conclusion", "Pour conclure",
        "Voici la réponse", "J'ai terminé", "Voilà",
        "N'hésite pas", "Si tu as d'autres", "Dis-moi si",
        "In summary", "In conclusion", "To summarize",
        "Here's the answer", "I've completed", "Let me know if",
    ];
    
    if final_indicators.iter().any(|ind| response.contains(ind)) {
        return true;
    }
    
    // No tool call JSON and reasonable length = probably final
    if !response.contains(r#"{"tool""#) && response.len() > 100 && response.len() < 2000 {
        return true;
    }
    
    false
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
