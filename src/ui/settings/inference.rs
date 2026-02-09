use crate::agent::{ExaSearchConfig, ExaSearchTool};
use crate::app::AppState;
use crate::storage::settings::save_settings;
use dioxus::prelude::*;
use std::sync::Arc;

pub fn InferenceSettings() -> Element {
    let app_state = use_context::<AppState>();
    let settings = app_state.settings.read().clone();
    let temperature = settings.temperature;
    let top_p = settings.top_p;
    let top_k = settings.top_k;
    let max_tokens = settings.max_tokens;
    let context_size = settings.context_size;
    let system_prompt = settings.system_prompt.clone();
    let exa_mcp_url = settings.exa_mcp_url.clone();
    let mut app_state_temperature = app_state.clone();
    let mut app_state_top_p = app_state.clone();
    let mut app_state_top_k = app_state.clone();
    let mut app_state_max_tokens = app_state.clone();
    let mut app_state_context_size = app_state.clone();
    let mut app_state_system_prompt = app_state.clone();
    let mut app_state_exa_mcp_url = app_state.clone();

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in-up pb-8",

            // Section: Generation Parameters — glass
            SettingsCard { title: "Generation Parameters",
                SettingsSlider {
                    label: "Temperature",
                    value: temperature,
                    min: 0.0,
                    max: 2.0,
                    step: 0.1,
                    description: "Controls randomness. Higher values make output more random.",
                    on_change: move |value| {
                        let mut settings = app_state_temperature.settings.write();
                        settings.temperature = value;
                        if let Err(error) = save_settings(&settings) {
                            tracing::error!("Failed to save settings: {}", error);
                        }
                    }
                }

                SettingsSlider {
                    label: "Top P",
                    value: top_p,
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                    description: "Nucleus sampling threshold.",
                    on_change: move |value| {
                        let mut settings = app_state_top_p.settings.write();
                        settings.top_p = value;
                        if let Err(error) = save_settings(&settings) {
                            tracing::error!("Failed to save settings: {}", error);
                        }
                    }
                }

                SettingsNumber {
                    label: "Top K",
                    value: top_k as f64,
                    min: 0.0,
                    max: 100.0,
                    description: "Limits token selection to K most likely tokens.",
                    on_change: move |value: f64| {
                        let mut settings = app_state_top_k.settings.write();
                        let clamped = value.clamp(0.0, 100.0).round() as u32;
                        settings.top_k = clamped;
                        if let Err(error) = save_settings(&settings) {
                            tracing::error!("Failed to save settings: {}", error);
                        }
                    }
                }
            }

            // Section: Model Configuration — glass
            SettingsCard { title: "Model Configuration",
                SettingsNumber {
                    label: "Max Tokens (Output)",
                    value: max_tokens as f64,
                    min: 256.0,
                    max: 16384.0,
                    description: "Tokens a generer. Plus petit = plus rapide. (Defaut: 4096)",
                    on_change: move |value: f64| {
                        let mut settings = app_state_max_tokens.settings.write();
                        settings.max_tokens = (value as u32).clamp(256, 16384);
                        if let Err(error) = save_settings(&settings) {
                            tracing::error!("Failed to save settings: {}", error);
                        }
                    }
                }

                // Context Size
                div { class: "mb-6",
                    div { class: "flex justify-between items-center mb-2",
                        label { class: "text-sm font-medium text-[var(--text-primary)]", "Context Window" }
                        span {
                            class: "text-xs px-2 py-0.5 rounded-md bg-[var(--bg-success-subtle)] text-[var(--text-success)] border border-[var(--border-success-subtle)]",
                            if context_size <= 8192 { "Rapide" } else if context_size <= 16384 { "Equilibre" } else { "Lent" }
                        }
                    }
                    select {
                        value: "{context_size}",
                        onchange: move |e| {
                            let value = e.value().parse().unwrap_or(8192);
                            let mut settings = app_state_context_size.settings.write();
                            settings.context_size = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full py-2.5 px-3 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:border-[var(--accent-primary)] transition-all outline-none text-sm appearance-none cursor-pointer",
                        option { value: "2048", "2K - Ultra rapide" }
                        option { value: "4096", "4K - Rapide" }
                        option { value: "8192", "8K - Recommande" }
                        option { value: "16384", "16K - Equilibre" }
                        option { value: "32768", "32K - Long contexte" }
                    }
                    p { class: "text-xs text-[var(--text-tertiary)] mt-1.5", "Taille du contexte. Plus petit = beaucoup plus rapide." }
                }

                // System Prompt Textarea
                div { class: "space-y-2",
                    label { class: "text-sm font-medium text-[var(--text-primary)]", "System Prompt" }
                    textarea {
                        value: "{system_prompt}",
                        oninput: move |e| {
                            let value = e.value();
                            let mut settings = app_state_system_prompt.settings.write();
                            settings.system_prompt = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full py-2.5 px-3 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:border-[var(--accent-primary)] transition-all outline-none text-sm h-28 resize-y",
                        placeholder: "Enter system prompt..."
                    }
                    p { class: "text-xs text-[var(--text-tertiary)]", "Initial instructions for the model's behavior." }
                }
            }

            // Section: Web Search (Exa MCP) — glass
            SettingsCard { title: "Web Search",
                div { class: "space-y-2",
                    label { class: "text-sm font-medium text-[var(--text-primary)]", "Exa MCP URL" }
                    input {
                        r#type: "text",
                        value: "{exa_mcp_url}",
                        oninput: move |e| {
                            let value = e.value();
                            let mut settings = app_state_exa_mcp_url.settings.write();
                            settings.exa_mcp_url = value.clone();
                            if value.is_empty() {
                                std::env::remove_var("EXA_MCP_URL");
                            } else {
                                std::env::set_var("EXA_MCP_URL", &value);
                            }
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }

                            let registry = app_state_exa_mcp_url.agent.tool_registry.clone();
                            let tool = ExaSearchTool::new(ExaSearchConfig {
                                mcp_url: value,
                                ..Default::default()
                            });
                            spawn(async move {
                                registry.register(Arc::new(tool)).await;
                            });
                        },
                        placeholder: "https://mcp.exa.ai/mcp",
                        class: "w-full py-2.5 px-3 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:border-[var(--accent-primary)] transition-all outline-none text-sm",
                    }
                    p { class: "text-xs text-[var(--text-tertiary)]",
                        "Pas besoin de cle. Tu peux ajouter ?exaApiKey=... en cas de rate limit."
                    }
                }
            }
        }
    }
}

#[component]
fn SettingsCard(title: &'static str, children: Element) -> Element {
    rsx! {
        div {
            class: "p-5 rounded-2xl glass-md",

            h3 {
                class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                "{title}"
            }

            {children}
        }
    }
}

#[component]
fn SettingsSlider(
    label: &'static str,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    description: &'static str,
    on_change: EventHandler<f32>,
) -> Element {
    rsx! {
        div { class: "mb-6",
            div { class: "flex justify-between items-center mb-2",
                label { class: "text-sm font-medium text-[var(--text-primary)]", "{label}" }
                span {
                    class: "text-xs font-mono px-2 py-1 rounded-lg bg-white/[0.04] text-[var(--text-secondary)] border border-[var(--border-subtle)]",
                    "{value:.2}"
                }
            }
            input {
                r#type: "range",
                min: "{min}",
                max: "{max}",
                step: "{step}",
                value: "{value}",
                oninput: move |e| {
                    let val = e.value().parse().unwrap_or(value);
                    on_change.call(val);
                },
                class: "w-full",
            }
            p { class: "text-xs text-[var(--text-tertiary)] mt-1.5", "{description}" }
        }
    }
}

#[component]
fn SettingsNumber(
    label: &'static str,
    value: f64,
    min: f64,
    max: f64,
    description: &'static str,
    on_change: EventHandler<f64>,
) -> Element {
    rsx! {
        div { class: "mb-6",
            label { class: "text-sm font-medium text-[var(--text-primary)] mb-2 block", "{label}" }
            input {
                r#type: "number",
                min: "{min}",
                max: "{max}",
                value: "{value}",
                oninput: move |e| {
                    let val = e.value().parse().unwrap_or(value);
                    on_change.call(val);
                },
                class: "w-full py-2.5 px-3 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:border-[var(--accent-primary)] transition-all outline-none text-sm",
            }
            p { class: "text-xs text-[var(--text-tertiary)] mt-1.5", "{description}" }
        }
    }
}
