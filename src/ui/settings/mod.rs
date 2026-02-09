#![allow(non_snake_case)]

pub mod appearance;
pub mod hardware;
pub mod inference;
pub mod tools;
pub mod mcp;

use crate::app::AppState;
use crate::ui::settings::appearance::AppearanceSettings;
use crate::ui::settings::hardware::HardwareSettings;
use crate::ui::settings::inference::InferenceSettings;
use crate::ui::settings::tools::ToolsSettings;
use crate::ui::settings::mcp::McpSettings;
use dioxus::prelude::*;

#[derive(PartialEq, Clone, Copy)]
enum SettingsTab {
    Inference,
    Hardware,
    Tools,
    Mcp,
    Appearance,
}

pub fn Settings() -> Element {
    let mut active_tab = use_signal(|| SettingsTab::Inference);
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";

    rsx! {
        div {
            class: "flex flex-col h-full min-h-0",

            // Header — glass pill tabs
            div {
                class: "flex-none px-6 py-4 border-b border-[var(--border-subtle)]",

                div {
                    class: "max-w-3xl mx-auto w-full",

                    // Tabs — glass pills
                    div {
                        class: "flex gap-1 p-1 rounded-xl w-fit",
                        style: "background: rgba(242,237,231,0.03); border: 1px solid rgba(242,237,231,0.06);",

                        TabButton {
                            active: active_tab() == SettingsTab::Inference,
                            onclick: move |_| active_tab.set(SettingsTab::Inference),
                            label: if is_en { "Inference" } else { "Inference" },
                        }
                        TabButton {
                            active: active_tab() == SettingsTab::Hardware,
                            onclick: move |_| active_tab.set(SettingsTab::Hardware),
                            label: if is_en { "Hardware" } else { "Materiel" },
                        }
                        TabButton {
                            active: active_tab() == SettingsTab::Tools,
                            onclick: move |_| active_tab.set(SettingsTab::Tools),
                            label: if is_en { "Tools" } else { "Outils" },
                        }
                        TabButton {
                            active: active_tab() == SettingsTab::Mcp,
                            onclick: move |_| active_tab.set(SettingsTab::Mcp),
                            label: "MCP",
                        }
                        TabButton {
                            active: active_tab() == SettingsTab::Appearance,
                            onclick: move |_| active_tab.set(SettingsTab::Appearance),
                            label: if is_en { "Appearance" } else { "Apparence" },
                        }
                    }
                }
            }

            // Content Area
            div {
                class: "flex-1 overflow-y-auto p-6 scrollbar-thin",
                match active_tab() {
                    SettingsTab::Inference => rsx! { InferenceSettings {} },
                    SettingsTab::Hardware => rsx! { HardwareSettings {} },
                    SettingsTab::Tools => rsx! { ToolsSettings {} },
                    SettingsTab::Mcp => rsx! { McpSettings {} },
                    SettingsTab::Appearance => rsx! { AppearanceSettings {} },
                }
            }
        }
    }
}

#[component]
fn TabButton(active: bool, onclick: EventHandler<MouseEvent>, label: String) -> Element {
    let classes = if active {
        "text-[var(--text-primary)] shadow-sm"
    } else {
        "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
    };

    rsx! {
        button {
            class: "py-2 px-4 rounded-lg text-sm font-medium transition-all {classes}",
            style: if active { "background: rgba(242,237,231,0.06); border: 1px solid rgba(242,237,231,0.08);" } else { "" },
            onclick: onclick,
            "{label}"
        }
    }
}
