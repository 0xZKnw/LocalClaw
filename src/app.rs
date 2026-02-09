//! Root Dioxus application component
//!
//! This module contains the main App component that serves as the root of the UI tree.

use crate::inference::LlamaEngine;
use crate::storage::conversations::Conversation;
use crate::storage::settings::{AppSettings, load_settings};
use crate::ui::Layout;
use crate::agent::{Agent, AgentConfig};
use dioxus::prelude::*;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::ui::chat::message::Message;

/// Represents the current state of the model
#[derive(Clone, PartialEq, Debug)]
pub enum ModelState {
    NotLoaded,
    Loading,
    Loaded(String),
    Error(String),
}

/// Global application state shared across components
#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<Agent>,
    pub engine: Arc<Mutex<LlamaEngine>>,
    pub current_conversation: Signal<Option<Conversation>>,
    pub conversations: Signal<Vec<Conversation>>,
    pub settings: Signal<AppSettings>,
    pub model_state: Signal<ModelState>,
    pub stop_signal: Arc<AtomicBool>,
    /// Global generation flag - generation continues even when navigating away
    pub is_generating: Signal<bool>,
    /// Active messages buffer - persists across navigation
    pub active_messages: Signal<Vec<Message>>,
}

impl AppState {
    pub fn new() -> Self {
        tracing::info!("AppState initialized");
        let settings = load_settings();
        let mut agent_config = AgentConfig::default();
        agent_config.disabled_mcp_servers = settings.disabled_mcp_servers.clone();
        
        Self {
            agent: Arc::new(Agent::new(agent_config)),
            engine: Arc::new(Mutex::new(LlamaEngine::new())),
            current_conversation: Signal::new(None),
            conversations: Signal::new(Vec::new()),
            settings: Signal::new(settings),
            model_state: Signal::new(ModelState::NotLoaded),
            stop_signal: Arc::new(AtomicBool::new(false)),
            is_generating: Signal::new(false),
            active_messages: Signal::new(Vec::new()),
        }
    }
}

#[component]
pub fn App() -> Element {
    let app_state = AppState::new();
    use_context_provider(|| app_state);

    {
        let agent = use_context::<AppState>().agent.clone();
        use_effect(move || {
            let agent = agent.clone();
            spawn(async move {
                if let Err(e) = agent.initialize_tools().await {
                    tracing::error!("Failed to initialize tools: {}", e);
                }
            });
        });
    }

    rsx! {
        Layout {}
    }
}
