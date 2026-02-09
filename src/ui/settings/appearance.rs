use crate::app::AppState;
use crate::storage::settings::{default_system_prompt_for_lang, save_settings};
use dioxus::prelude::*;

pub fn AppearanceSettings() -> Element {
    let app_state = use_context::<AppState>();
    let settings = app_state.settings.read().clone();
    let dark_mode = settings.theme == "dark";
    let current_lang = settings.language.clone();
    let is_fr = current_lang == "fr";
    let font_size = settings.font_size.to_lowercase();
    let selected_font_size = match font_size.as_str() {
        "small" => "Small",
        "large" => "Large",
        _ => "Medium",
    };
    let mut app_state_theme = app_state.clone();
    let mut app_state_font_size = app_state.clone();
    let mut app_state_lang = app_state.clone();

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in-up pb-8",

            // Language Card
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                    if is_fr { "Langue" } else { "Language" }
                }

                div {
                    div {
                        class: "text-sm font-medium text-[var(--text-primary)] mb-1",
                        if is_fr { "Langue de l'interface" } else { "Interface language" }
                    }
                    div {
                        class: "text-xs text-[var(--text-tertiary)] mb-4",
                        if is_fr { "Change la langue de l'interface et des réponses de l'IA" } else { "Changes the UI language and AI responses" }
                    }

                    div { class: "grid grid-cols-2 gap-3",
                        for (code, label, flag) in [("fr", "Français", "FR"), ("en", "English", "EN")] {
                            button {
                                onclick: {
                                    let code = code.to_string();
                                    move |_| {
                                        let mut settings = app_state_lang.settings.write();
                                        settings.language = code.clone();
                                        settings.system_prompt = default_system_prompt_for_lang(&code);
                                        if let Err(error) = save_settings(&settings) {
                                            tracing::error!("Failed to save settings: {}", error);
                                        }
                                    }
                                },
                                class: format!(
                                    "py-3 px-4 rounded-xl border transition-all text-center flex items-center justify-center gap-3 {}",
                                    if current_lang == code {
                                        "border-[var(--accent-primary)] bg-[var(--accent-primary-10)] text-[var(--accent-primary)]"
                                    } else {
                                        "border-[var(--border-subtle)] bg-white/[0.02] text-[var(--text-secondary)] hover:border-[var(--border-medium)] hover:bg-white/[0.04]"
                                    }
                                ),
                                span {
                                    class: "text-xs font-bold opacity-60",
                                    "{flag}"
                                }
                                span { class: "text-sm font-medium", "{label}" }
                            }
                        }
                    }
                }
            }

            // Theme Card — glass
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                    if is_fr { "Theme" } else { "Theme" }
                }

                div {
                    class: "flex items-center justify-between",

                    div {
                        div { class: "text-sm font-medium text-[var(--text-primary)]",
                            if is_fr { "Mode sombre" } else { "Dark Mode" }
                        }
                        div { class: "text-xs text-[var(--text-tertiary)] mt-0.5",
                            if is_fr { "Basculer entre le theme clair et sombre" } else { "Switch between light and dark theme" }
                        }
                    }
                    button {
                        onclick: move |_| {
                            let mut settings = app_state_theme.settings.write();
                            settings.theme = if dark_mode { "light".to_string() } else { "dark".to_string() };
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: if dark_mode { "toggle-switch active" } else { "toggle-switch" },
                        div { class: "toggle-switch-knob" }
                    }
                }
            }

            // Font Size Card — glass with selection cards
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                    if is_fr { "Typographie" } else { "Typography" }
                }

                div {
                    div { class: "text-sm font-medium text-[var(--text-primary)] mb-1",
                        if is_fr { "Taille de police" } else { "Font Size" }
                    }
                    div { class: "text-xs text-[var(--text-tertiary)] mb-4",
                        if is_fr { "Ajuster la taille du texte dans le chat" } else { "Adjust text size in the chat interface" }
                    }

                    div { class: "grid grid-cols-3 gap-3",
                        for size in &["Small", "Medium", "Large"] {
                            button {
                                onclick: move |_| {
                                    let mut settings = app_state_font_size.settings.write();
                                    settings.font_size = size.to_lowercase();
                                    if let Err(error) = save_settings(&settings) {
                                        tracing::error!("Failed to save settings: {}", error);
                                    }
                                },
                                class: format!(
                                    "py-3 px-4 rounded-xl border transition-all text-center {}",
                                    if selected_font_size == *size {
                                        "border-[var(--accent-primary)] bg-[var(--accent-primary-10)] text-[var(--accent-primary)]"
                                    } else {
                                        "border-[var(--border-subtle)] bg-white/[0.02] text-[var(--text-secondary)] hover:border-[var(--border-medium)] hover:bg-white/[0.04]"
                                    }
                                ),
                                div { class: "text-sm font-medium", "{size}" }
                                div {
                                    class: "text-[var(--text-tertiary)] mt-1",
                                    style: match *size {
                                        "Small" => "font-size: 0.75rem;",
                                        "Medium" => "font-size: 0.875rem;",
                                        "Large" => "font-size: 1rem;",
                                        _ => ""
                                    },
                                    "Aa"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
