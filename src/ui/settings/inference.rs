use crate::app::AppState;
use crate::storage::settings::save_settings;
use dioxus::prelude::*;

pub fn InferenceSettings() -> Element {
    let app_state = use_context::<AppState>();
    let settings = app_state.settings.read().clone();
    let temperature = settings.temperature;
    let top_p = settings.top_p;
    let top_k = settings.top_k;
    let max_tokens = settings.max_tokens;
    let context_size = settings.context_size;
    let system_prompt = settings.system_prompt.clone();
    let mut app_state_temperature = app_state.clone();
    let mut app_state_top_p = app_state.clone();
    let mut app_state_top_k = app_state.clone();
    let mut app_state_max_tokens = app_state.clone();
    let mut app_state_context_size = app_state.clone();
    let mut app_state_system_prompt = app_state.clone();

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in pb-8",

            // Section: Generation Parameters
            div {
                class: "p-6 rounded-2xl bg-white/[0.03] backdrop-blur-md border border-white/[0.08]",

                h3 {
                    class: "text-xl font-semibold mb-6 text-[var(--text-primary)]",
                    "Generation Parameters"
                }

                // Temperature Slider
                div {
                    class: "mb-6 space-y-3",

                    div { class: "flex justify-between items-center",
                        label { class: "font-medium text-[var(--text-primary)]", "Temperature" }
                        span {
                            class: "text-sm font-mono px-2 py-1 rounded bg-white/[0.05] text-[var(--text-secondary)] border border-white/[0.1]",
                            "{temperature:.2}"
                        }
                    }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "2",
                        step: "0.1",
                        value: "{temperature}",
                        oninput: move |e| {
                            let value = e.value().parse().unwrap_or(0.7);
                            let mut settings = app_state_temperature.settings.write();
                            settings.temperature = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full h-2 rounded-lg appearance-none cursor-pointer bg-white/[0.1]",
                        style: "accent-color: var(--accent-primary);"
                    }
                    p { class: "text-xs text-[var(--text-secondary)] opacity-70",
                        "Controls randomness. Higher values (e.g., 1.0) make output more random, while lower values (e.g., 0.2) make it more focused and deterministic."
                    }
                }

                // Top P Slider
                div {
                    class: "mb-6 space-y-3",

                    div { class: "flex justify-between items-center",
                        label { class: "font-medium text-[var(--text-primary)]", "Top P" }
                        span {
                            class: "text-sm font-mono px-2 py-1 rounded bg-white/[0.05] text-[var(--text-secondary)] border border-white/[0.1]",
                            "{top_p:.2}"
                        }
                    }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "1",
                        step: "0.05",
                        value: "{top_p}",
                        oninput: move |e| {
                            let value = e.value().parse().unwrap_or(0.9);
                            let mut settings = app_state_top_p.settings.write();
                            settings.top_p = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full h-2 rounded-lg appearance-none cursor-pointer bg-white/[0.1]",
                        style: "accent-color: var(--accent-primary);"
                    }
                    p { class: "text-xs text-[var(--text-secondary)] opacity-70",
                        "Nucleus sampling. Considers the smallest set of tokens whose cumulative probability exceeds the threshold P."
                    }
                }

                // Top K Input
                div { class: "space-y-2",
                    label { class: "font-medium block text-[var(--text-primary)]", "Top K" }
                    input {
                        r#type: "number",
                        min: "0",
                        max: "100",
                        value: "{top_k}",
                        oninput: move |e| {
                            let value = e.value().parse().unwrap_or(40);
                            let mut settings = app_state_top_k.settings.write();
                            settings.top_k = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full p-3 rounded-lg bg-white/[0.05] border border-white/[0.12] text-[var(--text-primary)] focus:border-[var(--accent-primary)] focus:ring-1 focus:ring-[var(--accent-primary)] transition-all outline-none",
                    }
                    p { class: "text-xs text-[var(--text-secondary)] opacity-70",
                        "Limits the next token selection to the K most likely tokens."
                    }
                }
            }

            // Section: Model Configuration
            div {
                class: "p-6 rounded-2xl bg-white/[0.03] backdrop-blur-md border border-white/[0.08]",

                h3 {
                    class: "text-xl font-semibold mb-6 text-[var(--text-primary)]",
                    "Model Configuration"
                }

                // Max Tokens Input
                div { class: "mb-6 space-y-2",
                    label { class: "font-medium block text-[var(--text-primary)]", "Max Tokens (Output)" }
                    input {
                        r#type: "number",
                        min: "1",
                        max: "65536",
                        value: "{max_tokens}",
                        oninput: move |e| {
                            let value = e.value().parse().unwrap_or(65536);
                            let mut settings = app_state_max_tokens.settings.write();
                            settings.max_tokens = value.clamp(1, 65536);
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full p-3 rounded-lg bg-white/[0.05] border border-white/[0.12] text-[var(--text-primary)] focus:border-[var(--accent-primary)] focus:ring-1 focus:ring-[var(--accent-primary)] transition-all outline-none",
                    }
                    p { class: "text-xs text-[var(--text-secondary)] opacity-70",
                        "Maximum number of tokens to generate in the response. Up to 64k tokens."
                    }
                }

                // Context Size Dropdown
                div { class: "mb-6 space-y-2",
                    label { class: "font-medium block text-[var(--text-primary)]", "Context Window Size" }
                    select {
                        value: "{context_size}",
                        onchange: move |e| {
                            let value = e.value().parse().unwrap_or(131072);
                            let mut settings = app_state_context_size.settings.write();
                            settings.context_size = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full p-3 rounded-lg bg-white/[0.05] border border-white/[0.12] text-[var(--text-primary)] focus:border-[var(--accent-primary)] focus:ring-1 focus:ring-[var(--accent-primary)] transition-all outline-none",
                        style: "color-scheme: dark;", // Ensures dropdown options are dark in dark mode
                        option { value: "2048", "2K Tokens" }
                        option { value: "4096", "4K Tokens" }
                        option { value: "8192", "8K Tokens" }
                        option { value: "16384", "16K Tokens" }
                        option { value: "32768", "32K Tokens" }
                        option { value: "65536", "64K Tokens" }
                        option { value: "131072", "128K Tokens (Default)" }
                    }
                    p { class: "text-xs text-[var(--text-secondary)] opacity-70",
                        "Maximum context window size. 128K allows for very long conversations and large inputs."
                    }
                }

                // System Prompt Textarea
                div { class: "space-y-2",
                    label { class: "font-medium block text-[var(--text-primary)]", "System Prompt" }
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
                        class: "w-full p-3 rounded-lg bg-white/[0.05] border border-white/[0.12] text-[var(--text-primary)] focus:border-[var(--accent-primary)] focus:ring-1 focus:ring-[var(--accent-primary)] transition-all outline-none h-32 resize-y font-sans",
                        placeholder: "Enter system prompt..."
                    }
                    p { class: "text-xs text-[var(--text-secondary)] opacity-70",
                        "The initial instructions given to the model to define its behavior and persona."
                    }
                }
            }
        }
    }
}
