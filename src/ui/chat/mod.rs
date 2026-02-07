//! Chat interface components
//!
//! Contains the main chat view, message display, and input components.
//! Implements an advanced agentic loop inspired by Claude Code and OpenCode.

pub mod input;
pub mod message;

use dioxus::prelude::*;
use input::ChatInput;
use message::{Message, MessageBubble, MessageRole};
use std::sync::atomic::Ordering;

use crate::agent::{
    extract_tool_call,
    format_tool_result_for_system,
    get_tool_permission,
    PermissionRequest,
    PermissionResult,
    PermissionDecision,
    AgentContext,
    AgentState,
};
use crate::agent::loop_runner::ToolHistoryEntry;
use crate::agent::tools::ToolResult;
use crate::agent::prompts::build_agent_system_prompt;
use crate::agent::prompts::build_reflection_prompt;
use crate::app::{AppState, ModelState};
use crate::inference::engine::GenerationParams;
use crate::inference::streaming::StreamToken;
use crate::storage::conversations::save_conversation;
use crate::types::message::{Message as StorageMessage, Role as StorageRole};
use chrono::Utc;
use uuid::Uuid;
use std::time::Instant;

#[component]
pub fn ChatView() -> Element {
    let app_state = use_context::<AppState>();
    
    // State for messages - will be populated from current_conversation
    let messages = use_signal(Vec::<Message>::new);
    
    // State for generation status
    let is_generating = use_signal(|| false);
    
    // Load messages when current_conversation changes
    {
        let mut messages = messages.clone();
        let current_conv = app_state.current_conversation.clone();
        use_effect(move || {
            let conv_read = current_conv.read();
            if let Some(ref conv) = *conv_read {
                if conv.messages.is_empty() {
                    // New conversation - start empty (no greeting)
                    messages.set(vec![]);
                } else {
                    // Load existing messages from storage
                    let ui_messages: Vec<Message> = conv.messages.iter()
                        .cloned()
                        .map(|m| m.into())
                        .collect();
                    messages.set(ui_messages);
                }
            }
        });
    }

    // Handler for sending a message
    let handle_send = {
        let mut messages = messages.clone();
        let mut is_generating = is_generating.clone();
        let app_state = app_state.clone();
        move |text: String| {
            if !matches!(*app_state.model_state.read(), ModelState::Loaded(_)) {
                messages.write().push(Message {
                    role: MessageRole::Assistant,
                    content: "Model not loaded. Please select and load a model first.".to_string(),
                });
                return;
            }

            // Add user message immediately
            messages.write().push(Message {
                role: MessageRole::User,
                content: text,
            });

            // Add empty assistant message to stream into
            messages.write().push(Message {
                role: MessageRole::Assistant,
                content: String::new(),
            });

            app_state.stop_signal.store(false, Ordering::Relaxed);
            is_generating.set(true);

            let mut messages = messages.clone();
            let mut is_generating = is_generating.clone();
            let mut app_state = app_state.clone();

            spawn(async move {
                // Initialize agent context for this run
                let mut agent_ctx = AgentContext::new();
                agent_ctx.state = AgentState::Analyzing;
                
                let (params, base_system_prompt, tools_enabled, tool_timeout_secs, max_iterations) = {
                    let settings = app_state.settings.read();
                    let params = GenerationParams {
                        max_tokens: settings.max_tokens,
                        temperature: settings.temperature,
                        top_k: settings.top_k,
                        top_p: settings.top_p,
                        repeat_penalty: 1.1,
                        seed: 0,
                        max_context_size: settings.context_size,
                    };

                    (
                        params,
                        settings.system_prompt.clone(),
                        app_state.agent.config.enable_tools,
                        app_state.agent.config.tool_timeout_secs,
                        app_state.agent.config.loop_config.max_iterations,
                    )
                };

                // Build the enhanced system prompt with tools
                let system_prompt = if tools_enabled {
                    let tools = app_state.agent.tool_registry.list_tools();
                    build_agent_system_prompt(&base_system_prompt, &tools, Some(&agent_ctx), None)
                } else {
                    base_system_prompt.clone()
                };

                // Advanced agent loop
                while agent_ctx.iteration < max_iterations {
                    agent_ctx.iteration += 1;

                    // Check stop signal
                    if app_state.stop_signal.load(Ordering::Relaxed) {
                        tracing::info!("Agent stopped by user at iteration {}", agent_ctx.iteration);
                        break;
                    }

                    // Check for stuck loop
                    if agent_ctx.is_stuck() {
                        let mut msgs = messages.write();
                        msgs.push(Message {
                            role: MessageRole::Assistant,
                            content: "⚠️ J'ai détecté que je répète les mêmes actions. Laisse-moi reformuler ma réponse.".to_string(),
                        });
                        break;
                    }

                    // Check max runtime (5 minutes)
                    if agent_ctx.elapsed().as_secs() > 300 {
                        let mut msgs = messages.write();
                        msgs.push(Message {
                            role: MessageRole::Assistant,
                            content: "⏱️ Temps d'exécution maximal atteint. Voici ce que j'ai trouvé jusqu'à présent.".to_string(),
                        });
                        break;
                    }

                    // Build context-aware prompt with tool history
                    let prompt_messages = {
                        let mut history = messages.read().clone();
                        if history
                            .last()
                            .map(|m| m.role == MessageRole::Assistant && m.content.is_empty())
                            .unwrap_or(false)
                        {
                            history.pop();
                        }

                        // Keep more history for better context
                        let max_history = 40usize;
                        if history.len() > max_history {
                            history = history[history.len() - max_history..].to_vec();
                        }

                        let mut prompt_messages: Vec<StorageMessage> = Vec::new();
                        
                        // System prompt with dynamic context injection
                        let dynamic_prompt = if agent_ctx.iteration > 1 && tools_enabled {
                            let tools = app_state.agent.tool_registry.list_tools();
                            build_agent_system_prompt(&base_system_prompt, &tools, Some(&agent_ctx), None)
                        } else {
                            system_prompt.clone()
                        };
                        
                        if !dynamic_prompt.trim().is_empty() {
                            prompt_messages.push(StorageMessage::new(
                                StorageRole::System,
                                dynamic_prompt,
                            ));
                        }
                        
                        prompt_messages.extend(history.into_iter().map(|m| m.into()));
                        prompt_messages
                    };

                    // Generate response
                    agent_ctx.state = AgentState::Thinking;
                    
                    let (rx, stop_signal) = {
                        let engine = app_state.engine.lock().await;
                        match engine.generate_stream_messages(prompt_messages, params.clone()) {
                            Ok(result) => result,
                            Err(e) => {
                                agent_ctx.consecutive_errors += 1;
                                messages.write().push(Message {
                                    role: MessageRole::Assistant,
                                    content: format!("❌ Erreur de génération: {e}"),
                                });
                                if agent_ctx.consecutive_errors >= 3 {
                                    break;
                                }
                                continue;
                            }
                        }
                    };

                    // Stream tokens - drain all available tokens per tick for smooth display
                    let mut stream_done = false;
                    while !stream_done {
                        if app_state.stop_signal.load(Ordering::Relaxed) {
                            stop_signal.store(true, Ordering::Relaxed);
                        }

                        // Drain all available tokens in one batch to reduce UI updates
                        let mut batch_text = String::new();
                        let mut got_any = false;
                        
                        loop {
                            match rx.try_recv() {
                                Ok(StreamToken::Token(text)) => {
                                    batch_text.push_str(&text);
                                    got_any = true;
                                }
                                Ok(StreamToken::Done) => {
                                    stream_done = true;
                                    break;
                                }
                                Ok(StreamToken::Error(e)) => {
                                    agent_ctx.consecutive_errors += 1;
                                    batch_text.push_str(&format!("\n\n❌ Erreur: {e}"));
                                    stream_done = true;
                                    break;
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    stream_done = true;
                                    break;
                                }
                            }
                        }
                        
                        // Apply all tokens in one write (reduces re-renders)
                        if !batch_text.is_empty() {
                            let mut msgs = messages.write();
                            if let Some(last) = msgs.last_mut() {
                                last.content.push_str(&batch_text);
                            }
                        }
                        
                        if !stream_done && !got_any {
                            // No tokens available, yield briefly
                            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                        }
                    }

                    // Check if stream ended with errors
                    let last_content = messages.read().last().map(|m| m.content.clone()).unwrap_or_default();
                    let had_stream_error = last_content.contains("❌ Erreur:");
                    
                    if had_stream_error {
                        // Stream error — give LLM a chance to recover
                        if agent_ctx.consecutive_errors < 3 {
                            messages.write().push(Message {
                                role: MessageRole::System,
                                content: "Une erreur est survenue pendant la génération. Reformule ta réponse ou essaie une approche différente.".to_string(),
                            });
                            messages.write().push(Message {
                                role: MessageRole::Assistant,
                                content: String::new(),
                            });
                            continue;
                        } else {
                            break;
                        }
                    }

                    // Reset consecutive errors on successful generation
                    agent_ctx.consecutive_errors = 0;

                    if !tools_enabled {
                        break;
                    }

                    // Extract and process tool call
                    agent_ctx.state = AgentState::Acting;
                    
                    let last_text = messages
                        .read()
                        .last()
                        .map(|m| m.content.clone())
                        .unwrap_or_default();

                    // Store last response for context
                    agent_ctx.last_response = Some(last_text.clone());

                    let tool_call = match extract_tool_call(&last_text) {
                        Some(call) => call,
                        None => {
                            // No tool call found — check if the LLM maybe tried but malformed the JSON
                            let looks_like_failed_json = last_text.contains("\"tool\"") || last_text.contains("{\"tool") || last_text.contains("```json");
                            
                            if looks_like_failed_json && agent_ctx.consecutive_errors < 2 {
                                // LLM tried to call a tool but the JSON was malformed
                                agent_ctx.consecutive_errors += 1;
                                messages.write().push(Message {
                                    role: MessageRole::System,
                                    content: "Le format JSON de l'appel d'outil était invalide. Rappel: utilise exactement ce format sans texte avant ni après:\n```json\n{\"tool\": \"nom_outil\", \"params\": {...}}\n```\nRéessaie avec le bon format.".to_string(),
                                });
                                messages.write().push(Message {
                                    role: MessageRole::Assistant,
                                    content: String::new(),
                                });
                                continue;
                            }
                            
                            // Genuine final response (no tool call intended)
                            agent_ctx.state = AgentState::Completed;
                            break;
                        }
                    };

                    // Show tool usage indicator
                    {
                        let mut msgs = messages.write();
                        if let Some(last) = msgs.last_mut() {
                            last.content = format!(
                                "🔧 Utilisation de l'outil `{}`... (itération {}/{})",
                                tool_call.tool, agent_ctx.iteration, max_iterations
                            );
                        }
                    }

                    // Permission check
                    let permission_level = get_tool_permission(&tool_call.tool);
                    let target = tool_call
                        .params
                        .get("path")
                        .and_then(|v| v.as_str())
                        .or_else(|| tool_call.params.get("query").and_then(|v| v.as_str()))
                        .or_else(|| tool_call.params.get("command").and_then(|v| v.as_str()))
                        .or_else(|| tool_call.params.get("url").and_then(|v| v.as_str()))
                        .or_else(|| tool_call.params.get("company_name").and_then(|v| v.as_str()))
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| tool_call.params.to_string());

                    let permission_request = PermissionRequest {
                        id: Uuid::new_v4(),
                        tool_name: tool_call.tool.clone(),
                        operation: "execute".to_string(),
                        target: target.clone(),
                        level: permission_level,
                        params: tool_call.params.clone(),
                        timestamp: Utc::now(),
                    };

                    // Check auto-approve settings before asking user
                    let auto_approved = {
                        let settings = app_state.settings.read();
                        settings.auto_approve_all_tools
                            || settings.tool_allowlist.contains(&tool_call.tool)
                    };

                    let permission_result = if auto_approved {
                        PermissionResult::Approved
                    } else {
                        app_state
                            .agent
                            .permission_manager
                            .request_permission(permission_request.clone())
                            .await
                    };

                    let approved = match permission_result {
                        PermissionResult::Approved => true,
                        PermissionResult::Pending => {
                            agent_ctx.state = AgentState::WaitingForUser;
                            {
                                let mut msgs = messages.write();
                                if let Some(last) = msgs.last_mut() {
                                    last.content = format!(
                                        "⏳ Autorisation requise pour `{}` ({}).\nCible: {}",
                                        tool_call.tool,
                                        permission_level.label(),
                                        target
                                    );
                                }
                            }

                            match app_state
                                .agent
                                .permission_manager
                                .wait_for_decision(
                                    permission_request.id,
                                    std::time::Duration::from_secs(120),
                                )
                                .await
                            {
                                Some(PermissionDecision::Approved) => true,
                                Some(PermissionDecision::Denied) => {
                                    let mut msgs = messages.write();
                                    if let Some(last) = msgs.last_mut() {
                                        last.content = format!(
                                            "🚫 Permission refusée pour `{}`.",
                                            tool_call.tool
                                        );
                                    }
                                    false
                                }
                                None => {
                                    let mut msgs = messages.write();
                                    if let Some(last) = msgs.last_mut() {
                                        last.content = format!(
                                            "⏱️ Délai expiré pour `{}`.",
                                            tool_call.tool
                                        );
                                    }
                                    false
                                }
                            }
                        }
                        PermissionResult::Denied => {
                            let mut msgs = messages.write();
                            if let Some(last) = msgs.last_mut() {
                                last.content = format!(
                                    "🚫 Permission refusée pour `{}`.",
                                    tool_call.tool
                                );
                            }
                            false
                        }
                    };

                    if !approved {
                        // Record denied permission in context and try alternative
                        agent_ctx.tool_history.push(ToolHistoryEntry {
                            tool_name: tool_call.tool.clone(),
                            params: tool_call.params.clone(),
                            result: None,
                            error: Some("Permission denied".to_string()),
                            timestamp: Utc::now().timestamp() as u64,
                            duration_ms: 0,
                        });
                        
                        // Add message to help LLM find alternative
                        messages.write().push(Message {
                            role: MessageRole::System,
                            content: format!(
                                "L'outil {} a été refusé. Essaie une autre approche ou réponds avec les informations disponibles.",
                                tool_call.tool
                            ),
                        });
                        messages.write().push(Message {
                            role: MessageRole::Assistant,
                            content: String::new(),
                        });
                        continue;
                    }

                    // Execute tool
                    let tool = match app_state.agent.tool_registry.get(&tool_call.tool) {
                        Some(tool) => tool,
                        None => {
                            agent_ctx.consecutive_errors += 1;
                            let mut msgs = messages.write();
                            if let Some(last) = msgs.last_mut() {
                                last.content = format!("❌ Outil introuvable: `{}`.", tool_call.tool);
                            }
                            // Let the LLM try a different tool
                            let available_tools: Vec<String> = app_state.agent.tool_registry.list_tools().iter().map(|t| t.name.clone()).collect();
                            msgs.push(Message {
                                role: MessageRole::System,
                                content: format!(
                                    "L'outil `{}` n'existe pas. Voici les outils disponibles: {}. Utilise un des outils existants ou réponds directement.",
                                    tool_call.tool,
                                    available_tools.join(", ")
                                ),
                            });
                            msgs.push(Message {
                                role: MessageRole::Assistant,
                                content: String::new(),
                            });
                            if agent_ctx.consecutive_errors >= 3 {
                                break;
                            }
                            continue;
                        }
                    };

                    let start_time = Instant::now();
                    let tool_result: Result<ToolResult, String> = match tokio::time::timeout(
                        std::time::Duration::from_secs(tool_timeout_secs),
                        tool.execute(tool_call.params.clone()),
                    )
                    .await
                    {
                        Ok(Ok(result)) => Ok(result),
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(_) => Err("Timeout dépassé".to_string()),
                    };
                    let duration_ms = start_time.elapsed().as_millis() as u64;

                    // Process result and update context
                    agent_ctx.state = AgentState::Observing;
                    
                    match tool_result {
                        Ok(result) => {
                            // Record success in history
                            agent_ctx.tool_history.push(ToolHistoryEntry {
                                tool_name: tool_call.tool.clone(),
                                params: tool_call.params.clone(),
                                result: Some(result.clone()),
                                error: None,
                                timestamp: Utc::now().timestamp() as u64,
                                duration_ms,
                            });

                            // Show result summary
                            let result_preview = if result.message.len() > 200 {
                                format!("{}...", &result.message[..200])
                            } else {
                                result.message.clone()
                            };
                            
                            messages.write().push(Message {
                                role: MessageRole::Assistant,
                                content: format!(
                                    "✅ `{}` ({:.1}s): {}",
                                    tool_call.tool,
                                    duration_ms as f64 / 1000.0,
                                    result_preview
                                ),
                            });

                            // Inject tool result for LLM
                            messages.write().push(Message {
                                role: MessageRole::System,
                                content: format_tool_result_for_system(&tool_call.tool, &result),
                            });

                            // Prepare for reflection/next iteration
                            agent_ctx.state = AgentState::Reflecting;
                            messages.write().push(Message {
                                role: MessageRole::Assistant,
                                content: String::new(),
                            });
                        }
                        Err(e) => {
                            // Record error in history
                            agent_ctx.tool_history.push(ToolHistoryEntry {
                                tool_name: tool_call.tool.clone(),
                                params: tool_call.params.clone(),
                                result: None,
                                error: Some(e.clone()),
                                timestamp: Utc::now().timestamp() as u64,
                                duration_ms,
                            });
                            
                            agent_ctx.consecutive_errors += 1;
                            
                            // Show error and inject reflection prompt
                            let error_msg = format!(
                                "❌ Erreur `{}`: {}",
                                tool_call.tool, e
                            );
                            
                            let mut msgs = messages.write();
                            if let Some(last) = msgs.last_mut() {
                                last.content = error_msg;
                            }
                            
                            // Give LLM a chance to recover
                            if agent_ctx.consecutive_errors < 4 {
                                msgs.push(Message {
                                    role: MessageRole::System,
                                    content: build_reflection_prompt(&tool_call.tool, &e, false),
                                });
                                msgs.push(Message {
                                    role: MessageRole::Assistant,
                                    content: String::new(),
                                });
                                agent_ctx.state = AgentState::Reflecting;
                            } else {
                                // Too many errors — add a final message explaining the situation
                                msgs.push(Message {
                                    role: MessageRole::System,
                                    content: format!(
                                        "Trop d'erreurs consécutives ({}). Arrête d'utiliser des outils et donne une réponse finale à l'utilisateur en expliquant ce que tu as essayé et ce qui n'a pas marché. Propose des solutions alternatives si possible.",
                                        agent_ctx.consecutive_errors
                                    ),
                                });
                                msgs.push(Message {
                                    role: MessageRole::Assistant,
                                    content: String::new(),
                                });
                                // One last generation attempt for the final message
                            }
                        }
                    }
                }

                is_generating.set(false);

                {
                    let mut msgs = messages.write();
                    if msgs
                        .last()
                        .map(|m| m.role == MessageRole::Assistant && m.content.is_empty())
                        .unwrap_or(false)
                    {
                        msgs.pop();
                    }
                }
                
                // Save messages to conversation after generation completes
                {
                    let msgs = messages.read();
                    let storage_messages: Vec<StorageMessage> = msgs.iter()
                        .cloned()
                        .map(|m| m.into())
                        .collect();
                    
                    let mut conv_write = app_state.current_conversation.write();
                    if let Some(ref mut conv) = *conv_write {
                        conv.messages = storage_messages;
                        if let Err(e) = save_conversation(conv) {
                            tracing::error!("Failed to save conversation: {}", e);
                        }
                    }
                }
            });
        }
    };

    // Handler for stopping generation
    let handle_stop = {
        let mut is_generating = is_generating.clone();
        let app_state = app_state.clone();
        move |_| {
            app_state.stop_signal.store(true, Ordering::Relaxed);
            is_generating.set(false);
        }
    };

    rsx! {
        div { class: "flex flex-col flex-1 min-h-0 relative",
            
            // Messages Area — narrower for readability
            div { class: "flex-1 min-h-0 overflow-y-auto px-4 py-4 custom-scrollbar scroll-smooth",
                div { class: "max-w-3xl mx-auto w-full flex flex-col gap-1 pb-4",
                    // Message List
                    for (idx, msg) in messages.read().iter().enumerate() {
                        if msg.role != MessageRole::System {
                            MessageBubble { key: "{idx}", message: msg.clone() }
                        }
                    }
                    
                    // Typing / Generating Indicator — softer dots
                    if is_generating() {
                        div { class: "message-layout",
                            div { class: "flex items-center gap-3 py-2 animate-fade-in",
                                div {
                                    class: "w-6 h-6 rounded-full flex items-center justify-center",
                                    style: "background: var(--accent-primary); opacity: 0.7;",
                                    div { class: "w-2 h-2 rounded-full animate-pulse", style: "background: #F2EDE7;" }
                                }
                                div { class: "flex items-center gap-1.5",
                                    div { class: "w-1.5 h-1.5 rounded-full bg-[var(--accent-primary)] opacity-60 animate-bounce" }
                                    div { class: "w-1.5 h-1.5 rounded-full bg-[var(--accent-primary)] opacity-60 animate-bounce delay-75" }
                                    div { class: "w-1.5 h-1.5 rounded-full bg-[var(--accent-primary)] opacity-60 animate-bounce delay-150" }
                                }
                            }
                        }
                    }
                    
                    div { class: "h-4" } // Spacer
                }
            }

            // Input Area
            ChatInput {
                on_send: handle_send,
                on_stop: handle_stop,
                is_generating: is_generating(),
            }
        }
    }
}
