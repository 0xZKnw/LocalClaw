//! UI components for LocalClaw
//!
//! This module contains all user interface components built with Dioxus.

pub mod chat;
pub mod components;
pub mod settings;
pub mod sidebar;

use crate::ui::sidebar::Sidebar;
use crate::ui::chat::ChatView;
use crate::ui::settings::Settings as SettingsPanel;
use crate::ui::components::permission_dialog::PermissionDialog;
use crate::app::{AppState, ModelState};
use crate::storage::models::scan_models_directory;
use dioxus::prelude::*;

/// Simple i18n helper — returns FR or EN string based on current language setting
pub fn t<'a>(app_state: &AppState, fr: &'a str, en: &'a str) -> &'a str {
    if app_state.settings.read().language == "en" { en } else { fr }
}

#[derive(Clone, Copy, PartialEq)]
enum MainView {
    Chat,
    Settings,
}

/// Compact model picker for the header bar
#[component]
fn HeaderModelPicker() -> Element {
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";
    let mut dropdown_open = use_signal(|| false);
    let mut models = use_signal(Vec::new);
    let models_directory = app_state.settings.read().models_directory.clone();

    // Scan models on mount
    let models_directory_clone = models_directory.clone();
    use_effect(move || {
        let found = scan_models_directory(&models_directory_clone).unwrap_or_default();
        models.set(found);
    });

    // Current state
    let model_state = app_state.model_state.read().clone();
    let is_loading = matches!(model_state, ModelState::Loading);
    let is_loaded = matches!(model_state, ModelState::Loaded(_));

    let display_name = match &model_state {
        ModelState::Loaded(path) => {
            std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| if s.len() > 20 { format!("{}...", crate::truncate_str(s, 20)) } else { s.to_string() })
                .unwrap_or_else(|| "Model".to_string())
        }
        ModelState::Loading => if is_en { "Loading..." } else { "Chargement..." }.to_string(),
        ModelState::Error(msg) => {
            let short = if msg.len() > 20 { format!("{}...", crate::truncate_str(&msg, 20)) } else { msg.clone() };
            format!("{}", short)
        }
        ModelState::NotLoaded => if is_en { "No model" } else { "Aucun modele" }.to_string(),
    };

    // Dot color class
    let dot_class = match &model_state {
        ModelState::Loaded(_) => "status-dot status-dot-ready",
        ModelState::Loading => "status-dot status-dot-loading",
        ModelState::Error(_) => "status-dot status-dot-error",
        ModelState::NotLoaded => "status-dot status-dot-idle",
    };

    // Handle load
    let app_state_load = app_state.clone();
    let handle_load = move |path: String| {
        let mut app_state = app_state_load.clone();
        dropdown_open.set(false);
        app_state.model_state.set(ModelState::Loading);
        let gpu_layers = app_state.settings.read().gpu_layers;
        spawn(async move {
            let result = {
                let mut engine = app_state.engine.lock().await;
                if !engine.is_initialized() {
                    if let Err(e) = engine.init() {
                        return app_state.model_state.set(ModelState::Error(e.to_string()));
                    }
                }
                engine.load_model_async(&path, gpu_layers).await
            };
            match result {
                Ok(_) => app_state.model_state.set(ModelState::Loaded(path)),
                Err(e) => app_state.model_state.set(ModelState::Error(e.to_string())),
            }
        });
    };

    // Handle unload
    let app_state_unload = app_state.clone();
    let handle_unload = move |_| {
        let mut app_state = app_state_unload.clone();
        dropdown_open.set(false);
        spawn(async move {
            let mut engine = app_state.engine.lock().await;
            engine.unload_model();
        });
        app_state.model_state.set(ModelState::NotLoaded);
    };

    rsx! {
        div {
            class: "relative",

            // Trigger pill button
            button {
                r#type: "button",
                onclick: move |_| if !is_loading { dropdown_open.set(!dropdown_open()) },
                class: "flex items-center gap-2 px-3 py-1.5 rounded-full hover:bg-white/[0.06] transition-all group",

                div { class: "{dot_class}" }

                // Loading state: show name + mini bar
                if is_loading {
                    div {
                        class: "flex flex-col items-start gap-0.5",
                        span {
                            class: "text-xs font-medium text-[var(--text-secondary)]",
                            "{display_name}"
                        }
                        div {
                            class: "loading-bar-mini",
                            style: "width: 80px;",
                        }
                    }
                } else {
                    span {
                        class: "text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)] transition-colors",
                        "{display_name}"
                    }
                    svg {
                        class: if dropdown_open() { "w-3 h-3 text-[var(--text-tertiary)] transition-transform rotate-180" } else { "w-3 h-3 text-[var(--text-tertiary)] transition-transform" },
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        polyline { points: "6 9 12 15 18 9" }
                    }
                }
            }

            // Dropdown panel
            if dropdown_open() {
                div {
                    class: "absolute left-1/2 mt-2 rounded-xl overflow-hidden z-50 animate-fade-in",
                    style: "transform: translateX(-50%); min-width: 260px; max-width: 340px; background: var(--bg-elevated); border: 1px solid var(--border-medium); box-shadow: 0 12px 32px -4px rgba(30,25,20,0.35);",

                    // Header
                    div {
                        class: "px-3 py-2 border-b border-[var(--border-subtle)]",
                        span {
                            class: "text-[10px] uppercase tracking-widest text-[var(--text-tertiary)] font-semibold",
                            if is_en { "Select Model" } else { "Choisir un modele" }
                        }
                    }

                    // Models list
                    div {
                        class: "max-h-56 overflow-y-auto custom-scrollbar py-1",

                        if models.read().is_empty() {
                            div {
                                class: "px-3 py-4 text-center",
                                span { class: "text-xs text-[var(--text-tertiary)]",
                                    if is_en { "No .gguf models found" } else { "Aucun modele .gguf trouve" }
                                }
                            }
                        }

                        for model in models.read().iter() {
                            {
                                let path_str = model.path.to_string_lossy().to_string();
                                let filename = model.filename.clone();
                                let size = model.size_string();
                                let is_current = match &model_state {
                                    ModelState::Loaded(p) => *p == path_str,
                                    _ => false,
                                };

                                rsx! {
                                    button {
                                        r#type: "button",
                                        onclick: {
                                            let path_str = path_str.clone();
                                            let mut handle_load = handle_load.clone();
                                            move |_| {
                                                if !is_current {
                                                    handle_load(path_str.clone());
                                                }
                                            }
                                        },
                                        class: "w-full flex items-center justify-between px-3 py-2 text-left text-sm transition-all hover:bg-white/[0.04]",
                                        style: if is_current {
                                            "background: var(--accent-soft); color: var(--accent-primary);"
                                        } else {
                                            "color: var(--text-primary);"
                                        },

                                        div {
                                            class: "flex items-center gap-2 min-w-0",
                                            if is_current {
                                                div { class: "w-1.5 h-1.5 rounded-full flex-shrink-0", style: "background: var(--accent-primary);" }
                                            }
                                            span { class: "truncate font-medium text-xs", "{filename}" }
                                        }
                                        span {
                                            class: "flex-shrink-0 text-[10px] font-mono text-[var(--text-tertiary)] ml-2",
                                            "{size}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Footer: Unload if loaded
                    if is_loaded {
                        div {
                            class: "px-2 py-2 border-t border-[var(--border-subtle)]",
                            button {
                                onclick: handle_unload,
                                class: "w-full flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-error-subtle)] hover:text-[var(--text-error)] transition-all",
                                svg {
                                    class: "w-3 h-3",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M18.36 6.64a9 9 0 1 1-12.73 0" }
                                    line { x1: "12", y1: "2", x2: "12", y2: "12" }
                                }
                                if is_en { "Unload model" } else { "Decharger le modele" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Prompt suggestion for welcome screen (bilingual)
struct PromptSuggestion {
    icon: &'static str,
    title_fr: &'static str,
    title_en: &'static str,
    subtitle_fr: &'static str,
    subtitle_en: &'static str,
    prompt_fr: &'static str,
    prompt_en: &'static str,
}

const SUGGESTIONS: &[PromptSuggestion] = &[
    PromptSuggestion {
        icon: "M9 20l-5.447-2.724A1 1 0 0 1 3 16.382V5.618a1 1 0 0 1 1.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0 0 21 18.382V7.618a1 1 0 0 0-.553-.894L15 4m0 13V4m0 0L9 7",
        title_fr: "Planifier",
        title_en: "Plan",
        subtitle_fr: "un voyage, un projet...",
        subtitle_en: "a trip, a project...",
        prompt_fr: "Aide-moi a planifier un voyage a Paris. Quels sont les incontournables et les meilleures periodes ?",
        prompt_en: "Help me plan a trip to Paris. What are the must-sees and best times to visit?",
    },
    PromptSuggestion {
        icon: "M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253",
        title_fr: "Expliquer",
        title_en: "Explain",
        subtitle_fr: "un concept complexe",
        subtitle_en: "a complex concept",
        prompt_fr: "Explique-moi l'informatique quantique en termes simples que n'importe qui peut comprendre.",
        prompt_en: "Explain quantum computing in simple terms that anyone can understand.",
    },
    PromptSuggestion {
        icon: "M11 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-5m-1.414-9.414a2 2 0 1 1 2.828 2.828L11.828 15H9v-2.828l8.586-8.586z",
        title_fr: "Rediger",
        title_en: "Write",
        subtitle_fr: "un email, un texte...",
        subtitle_en: "an email, a document...",
        prompt_fr: "Aide-moi a ecrire un email professionnel a mon manager pour demander des conges.",
        prompt_en: "Help me write a professional email to my manager asking for time off.",
    },
    PromptSuggestion {
        icon: "M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4",
        title_fr: "Coder",
        title_en: "Code",
        subtitle_fr: "debugger, expliquer...",
        subtitle_en: "debug, explain...",
        prompt_fr: "J'ai un bug dans mon code. Peux-tu m'aider a le debugger ?",
        prompt_en: "I have a bug in my code. Can you help me debug it?",
    },
];

/// Main Application Layout
#[component]
pub fn Layout() -> Element {
    let mut current_view = use_signal(|| MainView::Chat);
    let mut sidebar_visible = use_signal(|| true);
    let app_state = use_context::<AppState>();
    
    // Get theme from settings
    let theme_str = app_state.settings.read().theme.clone();
    let is_en = app_state.settings.read().language == "en";

    rsx! {
        // Theme wrapper
        div {
            "data-theme": "{theme_str}",
            class: "relative flex h-screen w-screen bg-[var(--bg-primary)] text-[var(--text-primary)] overflow-hidden",

            // Inline CSS
            style { {include_str!("../../assets/styles.css")} }

            // Ambient gradient orbs (behind everything)
            div { class: "ambient-orb ambient-orb-1" }
            div { class: "ambient-orb ambient-orb-2" }
            div { class: "ambient-orb ambient-orb-3" }

            // Noise overlay
            div { class: "noise-overlay" }

            // Sidebar (collapsible)
            if sidebar_visible() {
                Sidebar {
                    on_settings_click: move |_| current_view.set(MainView::Settings),
                    on_new_chat: move |_| current_view.set(MainView::Chat)
                }
            }

            // Main Content Area
            div {
                class: "flex-1 flex flex-col h-full min-h-0 relative min-w-0 z-10",

                // Header Bar — transparent, blends with background
                div {
                    class: "flex-none h-11 flex items-center justify-between px-3 border-b border-[var(--border-subtle)]",
                    style: "background: var(--bg-primary);",

                    // Left: Toggle sidebar + New chat
                    div {
                        class: "flex items-center gap-1",

                        button {
                            onclick: move |_| sidebar_visible.set(!sidebar_visible()),
                            class: "w-8 h-8 rounded-lg hover:bg-white/[0.06] flex items-center justify-center text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-all",
                            title: if sidebar_visible() { if is_en { "Hide" } else { "Masquer" } } else { if is_en { "Show" } else { "Afficher" } },
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "1.5",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                rect { x: "3", y: "3", width: "18", height: "18", rx: "2" }
                                line { x1: "9", y1: "3", x2: "9", y2: "21" }
                            }
                        }

                        button {
                            onclick: {
                                let mut current_conversation = app_state.current_conversation.clone();
                                let mut conversations = app_state.conversations.clone();
                                move |_| {
                                    use crate::storage::conversations::{save_conversation, list_conversations, Conversation};
                                    let conversation = Conversation::new(None);
                                    if let Err(e) = save_conversation(&conversation) {
                                        tracing::error!("Failed to save conversation: {}", e);
                                        return;
                                    }
                                    current_conversation.set(Some(conversation));
                                    if let Ok(convs) = list_conversations() {
                                        conversations.set(convs);
                                    }
                                    current_view.set(MainView::Chat);
                                }
                            },
                            class: "w-8 h-8 rounded-lg hover:bg-white/[0.06] flex items-center justify-center text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-all",
                            title: if is_en { "New chat" } else { "Nouveau chat" },
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "1.5",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" }
                                path { d: "M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" }
                            }
                        }
                    }

                    // Center: Model picker dropdown
                    HeaderModelPicker {}

                    // Right: Settings
                    button {
                        onclick: move |_| current_view.set(MainView::Settings),
                        class: "w-8 h-8 rounded-lg hover:bg-white/[0.06] flex items-center justify-center text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-all",
                        title: "Parametres",
                        svg {
                            width: "15",
                            height: "15",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.5",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            circle { cx: "12", cy: "12", r: "3" }
                            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" }
                        }
                    }
                }

                // Main Content
                if current_view() == MainView::Settings {
                    div {
                        class: "flex flex-col h-full",
                        // Back Button Header
                        div {
                            class: "flex-none px-6 pt-4 pb-2",
                            button {
                                onclick: move |_| current_view.set(MainView::Chat),
                                class: "flex items-center gap-2 text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors text-sm font-medium group",
                                svg {
                                    class: "w-4 h-4 transition-transform group-hover:-translate-x-1",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M19 12H5M12 19l-7-7 7-7" }
                                }
                                "Back to Chat"
                            }
                        }
                        SettingsPanel {}
                    }
                } else if app_state.current_conversation.read().is_some() {
                    ChatView {}
                } else {
                    WelcomeScreen {
                        on_prompt_click: {
                            let mut current_conversation = app_state.current_conversation.clone();
                            let mut conversations = app_state.conversations.clone();
                            move |_prompt: String| {
                                use crate::storage::conversations::{save_conversation, list_conversations, Conversation};
                                let conversation = Conversation::new(None);
                                if let Err(e) = save_conversation(&conversation) {
                                    tracing::error!("Failed to save conversation: {}", e);
                                    return;
                                }
                                current_conversation.set(Some(conversation));
                                if let Ok(convs) = list_conversations() {
                                    conversations.set(convs);
                                }
                            }
                        }
                    }
                }
            }

            PermissionDialog {}
        }
    }
}

/// Welcome screen with premium gradient orb and prompt suggestions grid
#[component]
fn WelcomeScreen(on_prompt_click: EventHandler<String>) -> Element {
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";
    rsx! {
        div {
            class: "flex-1 flex flex-col relative overflow-hidden",

            // Soft radial background glow
            div {
                class: "absolute inset-0 pointer-events-none",
                style: "background: radial-gradient(ellipse 60% 40% at 50% 35%, rgba(42,107,124,0.06), transparent 70%);"
            }

            // Main centered content
            div {
                class: "flex-1 flex flex-col items-center justify-center px-6 relative z-10",

                // Stylised title — minimal, no icon
                div {
                    class: "mb-12 flex flex-col items-center animate-fade-in",

                    h1 {
                        class: "font-bold tracking-tight mb-2",
                        style: "font-size: 3.5rem; letter-spacing: -0.04em; line-height: 1;",

                        // "Loca" in primary text color
                        span {
                            class: "text-[var(--text-primary)]",
                            style: "font-weight: 300;",
                            "Loca"
                        }
                        // "LM" in accent color, heavier weight
                        span {
                            style: "color: var(--accent-primary); font-weight: 700;",
                            "LM"
                        }
                    }

                    // Thin separator line
                    div {
                        style: "width: 48px; height: 2px; background: var(--accent-primary); opacity: 0.4; border-radius: 1px; margin-bottom: 0.75rem;",
                    }

                    // Subtitle
                    p {
                        class: "text-[var(--text-tertiary)] text-center text-sm tracking-wide",
                        style: "letter-spacing: 0.12em; text-transform: uppercase; font-weight: 500;",
                        if is_en { "Your private AI, 100% local" } else { "Votre IA privee, 100% locale" }
                    }
                }

                // 2x2 Suggestions grid — glass cards
                div {
                    class: "grid grid-cols-2 gap-3 w-full max-w-xl mb-8",

                    for (i, suggestion) in SUGGESTIONS.iter().enumerate() {
                        {
                            let title = if is_en { suggestion.title_en } else { suggestion.title_fr };
                            let subtitle = if is_en { suggestion.subtitle_en } else { suggestion.subtitle_fr };
                            let prompt = if is_en { suggestion.prompt_en } else { suggestion.prompt_fr };
                            rsx! {
                                button {
                                    onclick: {
                                        let prompt = prompt.to_string();
                                        let on_prompt_click = on_prompt_click.clone();
                                        move |_| {
                                            on_prompt_click.call(prompt.clone());
                                        }
                                    },
                                    class: "flex items-start gap-3 px-4 py-3.5 rounded-2xl glass glass-hover transition-all text-left group cursor-pointer animate-fade-in-up",
                                    style: format!("animation-delay: {}ms; animation-fill-mode: both;", 200 + i * 75),

                                    // Icon circle
                                    div {
                                        class: "flex-shrink-0 w-9 h-9 rounded-xl bg-[var(--accent-primary-10)] flex items-center justify-center mt-0.5",
                                        svg {
                                            class: "w-4 h-4 text-[var(--accent-primary)]",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "1.5",
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            path { d: "{suggestion.icon}" }
                                        }
                                    }

                                    // Text
                                    div {
                                        class: "flex flex-col min-w-0",
                                        span {
                                            class: "text-sm font-semibold text-[var(--text-primary)] group-hover:text-[var(--accent-primary)] transition-colors leading-tight",
                                            "{title}"
                                        }
                                        span {
                                            class: "text-xs text-[var(--text-tertiary)] mt-0.5 leading-snug",
                                            "{subtitle}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Bottom: Clean input CTA
            div {
                class: "px-6 pb-8",

                div {
                    class: "max-w-xl mx-auto",

                    button {
                        onclick: {
                            let on_prompt_click = on_prompt_click.clone();
                            move |_| {
                                on_prompt_click.call(String::new());
                            }
                        },
                        class: "w-full flex items-center gap-3 py-3.5 px-5 rounded-3xl glass glass-hover transition-all cursor-text group animate-fade-in-up",
                        style: "animation-delay: 500ms; animation-fill-mode: both;",

                        span {
                            class: "flex-1 text-left text-[var(--text-tertiary)] text-[15px]",
                            if is_en { "Send a message..." } else { "Envoyer un message..." }
                        }

                        // Send arrow
                        div {
                            class: "flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center opacity-50 group-hover:opacity-100 transition-all",
                            style: "background: var(--accent-primary);",
                            svg {
                                class: "w-4 h-4",
                                style: "color: #F2EDE7;",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                line { x1: "12", y1: "19", x2: "12", y2: "5" }
                                polyline { points: "5 12 12 5 19 12" }
                            }
                        }
                    }

                    // Privacy badge
                    p {
                        class: "text-center text-xs text-[var(--text-tertiary)] mt-3 opacity-40",
                        if is_en { "100% private — no data leaves your device" } else { "100% prive — aucune donnee ne quitte votre appareil" }
                    }
                }
            }
        }
    }
}
