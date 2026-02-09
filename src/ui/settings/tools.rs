use crate::agent::get_tool_permission;
use crate::app::AppState;
use crate::storage::settings::save_settings;
use dioxus::prelude::*;

/// Known tool groups for the allowlist UI
const TOOL_GROUPS: &[(&str, &[&str], &str, &str)] = &[
    // (group_label_en, tool_names, icon, risk_level)
    (
        "File Read",
        &[
            "file_read",
            "file_list",
            "grep",
            "glob",
            "file_info",
            "file_search",
        ],
        "📂",
        "safe",
    ),
    (
        "File Write",
        &[
            "file_write",
            "file_edit",
            "file_create",
            "file_delete",
            "file_move",
            "file_copy",
            "directory_create",
        ],
        "📝",
        "moderate",
    ),
    (
        "Shell / Bash",
        &["bash", "bash_background", "command"],
        "⚡",
        "dangerous",
    ),
    (
        "Git",
        &[
            "git_status",
            "git_diff",
            "git_log",
            "git_commit",
            "git_branch",
            "git_stash",
        ],
        "🔀",
        "moderate",
    ),
    (
        "Web / Network",
        &[
            "web_search",
            "code_search",
            "company_research",
            "web_fetch",
            "web_download",
            "web_crawl",
        ],
        "🌐",
        "moderate",
    ),
    (
        "Dev Tools",
        &["diff", "find_replace", "patch", "wc"],
        "🛠️",
        "safe",
    ),
    (
        "System",
        &[
            "process_list",
            "environment",
            "system_info",
            "which",
            "tree",
        ],
        "💻",
        "safe",
    ),
];

const TOOL_GROUPS_FR: &[&str] = &[
    "Lecture fichiers",
    "Ecriture fichiers",
    "Shell / Bash",
    "Git",
    "Web / Reseau",
    "Outils dev",
    "Systeme",
];

pub fn ToolsSettings() -> Element {
    let app_state = use_context::<AppState>();
    let settings = app_state.settings.read().clone();
    let is_en = settings.language == "en";
    let auto_approve = settings.auto_approve_all_tools;
    let allowlist = settings.tool_allowlist.clone();

    let mut app_state_toggle = app_state.clone();
    let mut app_state_group = app_state.clone();
    let mut app_state_tool = app_state.clone();

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in-up pb-8",

            // Auto-approve ALL toggle
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-1 text-[var(--text-primary)]",
                    if is_en { "Auto-approve Mode" } else { "Mode tout accepter" }
                }
                p {
                    class: "text-xs text-[var(--text-tertiary)] mb-5",
                    if is_en {
                        "When enabled, ALL tool calls are automatically approved without asking. Use with caution."
                    } else {
                        "Quand active, TOUS les appels d'outils sont approuves automatiquement. A utiliser avec precaution."
                    }
                }

                div {
                    class: "flex items-center justify-between",

                    div {
                        div {
                            class: "text-sm font-medium text-[var(--text-primary)] flex items-center gap-2",
                            if is_en { "Accept all tools" } else { "Tout accepter" }
                            if auto_approve {
                                span {
                                    class: "px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase",
                                    style: "background: rgba(196,69,69,0.12); color: #C45B5B;",
                                    if is_en { "DANGEROUS" } else { "DANGEREUX" }
                                }
                            }
                        }
                        div {
                            class: "text-xs text-[var(--text-tertiary)] mt-0.5",
                            if is_en { "Skip permission dialogs for all tools" } else { "Ignorer les dialogues de permission pour tous les outils" }
                        }
                    }
                    button {
                        onclick: move |_| {
                            let mut settings = app_state_toggle.settings.write();
                            settings.auto_approve_all_tools = !settings.auto_approve_all_tools;
                            if let Err(e) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", e);
                            }
                        },
                        class: if auto_approve { "toggle-switch active" } else { "toggle-switch" },
                        div { class: "toggle-switch-knob" }
                    }
                }
            }

            // Allowlist — per-group and per-tool toggles
            if !auto_approve {
                div {
                    class: "p-5 rounded-2xl glass-md",

                    h3 {
                        class: "text-base font-semibold mb-1 text-[var(--text-primary)]",
                        if is_en { "Tool Allowlist" } else { "Liste d'outils autorises" }
                    }
                    p {
                        class: "text-xs text-[var(--text-tertiary)] mb-5",
                        if is_en {
                            "Tools in the allowlist are auto-approved. Others will require manual approval."
                        } else {
                            "Les outils dans la liste sont approuves automatiquement. Les autres demanderont une approbation manuelle."
                        }
                    }

                    div {
                        class: "space-y-3",

                        for (idx, (label_en, tools, icon, risk)) in TOOL_GROUPS.iter().enumerate() {
                            {
                                let label = if is_en { label_en.to_string() } else { TOOL_GROUPS_FR[idx].to_string() };
                                let tools_vec: Vec<String> = tools.iter().map(|t| t.to_string()).collect();
                                let all_in_allowlist = tools_vec.iter().all(|t| allowlist.contains(t));
                                let some_in_allowlist = tools_vec.iter().any(|t| allowlist.contains(t));

                                let risk_style = match *risk {
                                    "dangerous" => "background: rgba(196,69,69,0.10); color: #C45B5B; border: 1px solid rgba(196,69,69,0.15);",
                                    "moderate" => "background: rgba(196,153,59,0.10); color: #C4993B; border: 1px solid rgba(196,153,59,0.15);",
                                    _ => "background: rgba(90,158,124,0.10); color: #5A9E7C; border: 1px solid rgba(90,158,124,0.15);",
                                };
                                let risk_label = match (*risk, is_en) {
                                    ("dangerous", true) => "high risk",
                                    ("dangerous", false) => "risque eleve",
                                    ("moderate", true) => "moderate",
                                    ("moderate", false) => "modere",
                                    (_, true) => "safe",
                                    (_, false) => "sur",
                                };

                                {
                                    // Pre-compute checkbox style to avoid type inference issues
                                    let checkbox_style = if all_in_allowlist {
                                        "background: var(--accent-primary); border-color: var(--accent-primary);"
                                    } else if some_in_allowlist {
                                        "background: var(--accent-soft); border-color: var(--accent-primary);"
                                    } else {
                                        "border-color: var(--border-medium);"
                                    };

                                    rsx! {
                                        div {
                                            class: "rounded-xl border border-[var(--border-subtle)] overflow-hidden",

                                            // Group header — clickable to toggle entire group
                                            button {
                                                r#type: "button",
                                                onclick: {
                                                    let tools_vec = tools_vec.clone();
                                                    let all_in = all_in_allowlist;
                                                    move |_| {
                                                        let mut settings = app_state_group.settings.write();
                                                        if all_in {
                                                            settings.tool_allowlist.retain(|t| !tools_vec.contains(t));
                                                        } else {
                                                            for t in &tools_vec {
                                                                if !settings.tool_allowlist.contains(t) {
                                                                    settings.tool_allowlist.push(t.clone());
                                                                }
                                                            }
                                                        }
                                                        if let Err(e) = save_settings(&settings) {
                                                            tracing::error!("Failed to save settings: {}", e);
                                                        }
                                                    }
                                                },
                                                class: "w-full flex items-center justify-between px-4 py-3 text-left hover:bg-white/[0.03] transition-all",

                                                div {
                                                    class: "flex items-center gap-3",
                                                    span { class: "text-base", "{icon}" }
                                                    div {
                                                        span { class: "text-sm font-medium text-[var(--text-primary)]", "{label}" }
                                                        span {
                                                            class: "ml-2 px-1.5 py-0.5 rounded text-[9px] font-semibold uppercase",
                                                            style: "{risk_style}",
                                                            "{risk_label}"
                                                        }
                                                    }
                                                }

                                                div {
                                                    class: "w-5 h-5 rounded-md border flex items-center justify-center flex-shrink-0 transition-all",
                                                    style: "{checkbox_style}",
                                                    if all_in_allowlist {
                                                        svg {
                                                            class: "w-3 h-3",
                                                            style: "color: #F2EDE7;",
                                                            view_box: "0 0 24 24",
                                                            fill: "none",
                                                            stroke: "currentColor",
                                                            stroke_width: "3",
                                                            stroke_linecap: "round",
                                                            stroke_linejoin: "round",
                                                            polyline { points: "20 6 9 17 4 12" }
                                                        }
                                                    } else if some_in_allowlist {
                                                        div {
                                                            class: "w-2 h-0.5 rounded-full",
                                                            style: "background: var(--accent-primary);",
                                                        }
                                                    }
                                                }
                                            }

                                            // Individual tools
                                            div {
                                                class: "border-t border-[var(--border-subtle)] bg-white/[0.01]",

                                                for tool_name in tools.iter() {
                                                    {
                                                        let tool = tool_name.to_string();
                                                        let is_allowed = allowlist.contains(&tool);
                                                        let perm = get_tool_permission(tool_name);
                                                        let tool_cb_style = if is_allowed {
                                                            "background: var(--accent-primary); border-color: var(--accent-primary);"
                                                        } else {
                                                            "border-color: var(--border-medium);"
                                                        };

                                                        rsx! {
                                                            button {
                                                                r#type: "button",
                                                                onclick: {
                                                                    let tool = tool.clone();
                                                                    move |_| {
                                                                        let mut settings = app_state_tool.settings.write();
                                                                        if settings.tool_allowlist.contains(&tool) {
                                                                            settings.tool_allowlist.retain(|t| t != &tool);
                                                                        } else {
                                                                            settings.tool_allowlist.push(tool.clone());
                                                                        }
                                                                        if let Err(e) = save_settings(&settings) {
                                                                            tracing::error!("Failed to save settings: {}", e);
                                                                        }
                                                                    }
                                                                },
                                                                class: "w-full flex items-center justify-between px-4 py-2 text-left hover:bg-white/[0.03] transition-all",

                                                                div {
                                                                    class: "flex items-center gap-2",
                                                                    span {
                                                                        class: "text-xs font-mono text-[var(--text-secondary)]",
                                                                        "{tool_name}"
                                                                    }
                                                                    span {
                                                                        class: "text-[9px] text-[var(--text-tertiary)]",
                                                                        "({perm})",
                                                                    }
                                                                }

                                                                div {
                                                                    class: "w-4 h-4 rounded border flex items-center justify-center flex-shrink-0 transition-all",
                                                                    style: "{tool_cb_style}",
                                                                    if is_allowed {
                                                                        svg {
                                                                            class: "w-2.5 h-2.5",
                                                                            style: "color: #F2EDE7;",
                                                                            view_box: "0 0 24 24",
                                                                            fill: "none",
                                                                            stroke: "currentColor",
                                                                            stroke_width: "3",
                                                                            stroke_linecap: "round",
                                                                            stroke_linejoin: "round",
                                                                            polyline { points: "20 6 9 17 4 12" }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
