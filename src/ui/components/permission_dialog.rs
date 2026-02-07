//! Permission dialog UI component
//!
//! Displays permission requests and allows user approval/denial

use crate::agent::permissions::PermissionLevel;
use crate::app::AppState;
use dioxus::prelude::*;

/// Permission dialog component
#[component]
pub fn PermissionDialog() -> Element {
    let app_state = use_context::<AppState>();
    let signals = app_state.agent.permission_manager.signals();
    let requests = signals.pending_requests.read();

    if requests.is_empty() {
        return rsx! { div {} };
    }

    let current_request = &requests[0];
    let request_id = current_request.id;
    let manager = app_state.agent.permission_manager.clone();
    let manager_deny = manager.clone();
    let manager_approve = manager.clone();
    let is_en = app_state.settings.read().language == "en";

    rsx! {
        // Backdrop — heavy blur
        div {
            class: "fixed inset-0 bg-black/60 backdrop-blur-2xl z-50 flex items-center justify-center p-4",

            // Dialog — glass-strong with spring animation
            div {
                class: "w-full max-w-lg glass-strong rounded-2xl overflow-hidden animate-scale-in",

                // Header — with warning icon
                div {
                    class: "p-6 border-b border-[var(--border-subtle)]",

                    div {
                        class: "flex items-center gap-3 mb-2",

                        div {
                            class: "w-10 h-10 rounded-full flex items-center justify-center",
                            style: "background: rgba(196,153,59,0.12); border: 1px solid rgba(196,153,59,0.2);",
                            svg {
                                class: "w-5 h-5",
                                style: "color: #C4993B;",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" }
                                line { x1: "12", y1: "9", x2: "12", y2: "13" }
                                line { x1: "12", y1: "17", x2: "12.01", y2: "17" }
                            }
                        }

                        h2 {
                            class: "text-lg font-semibold text-[var(--text-primary)]",
                            if is_en { "Permission Required" } else { "Permission requise" }
                        }
                    }

                    p {
                        class: "text-sm text-[var(--text-secondary)]",
                        if is_en { "The AI agent is requesting permission to perform an action." } else { "L'agent IA demande la permission d'effectuer une action." }
                    }
                }

                // Content
                div {
                    class: "p-6 space-y-3",

                    // Tool info — glass card
                    div {
                        class: "p-4 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)]",

                        div {
                            class: "flex items-center justify-between mb-2",
                            span { class: "text-sm font-medium text-[var(--text-secondary)]",
                                if is_en { "Tool" } else { "Outil" }
                            }
                            span { class: "text-sm text-[var(--accent-primary)] font-medium", "{current_request.tool_name}" }
                        }

                        div {
                            class: "flex items-center justify-between mb-2",
                            span { class: "text-sm font-medium text-[var(--text-secondary)]",
                                if is_en { "Operation" } else { "Operation" }
                            }
                            span { class: "text-sm text-[var(--text-primary)]", "{current_request.operation}" }
                        }

                        div {
                            class: "flex items-center justify-between",
                            span { class: "text-sm font-medium text-[var(--text-secondary)]",
                                if is_en { "Level" } else { "Niveau" }
                            }
                            PermissionLevelBadge { level: current_request.level }
                        }
                    }

                    // Target — glass card
                    div {
                        class: "p-4 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)]",
                        span { class: "text-[10px] uppercase tracking-widest text-[var(--text-tertiary)] font-semibold",
                            if is_en { "Target" } else { "Cible" }
                        }
                        p { class: "mt-1 text-sm font-mono text-[var(--text-secondary)] break-all", "{current_request.target}" }
                    }

                    // Parameters
                    details {
                        class: "p-4 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)]",
                        summary { class: "text-[10px] uppercase tracking-widest text-[var(--text-tertiary)] font-semibold cursor-pointer",
                            if is_en { "Parameters" } else { "Parametres" }
                        }
                        pre { class: "mt-2 text-xs text-[var(--text-secondary)] overflow-x-auto font-mono", "{serde_json::to_string_pretty(&current_request.params).unwrap_or_default()}" }
                    }
                }

                // Footer — glass buttons
                div {
                    class: "p-6 border-t border-[var(--border-subtle)] flex gap-3",

                    button {
                        class: "btn-ghost flex-1",
                        onclick: move |_| {
                            let manager = manager_deny.clone();
                            spawn(async move {
                                let _ = manager.deny(request_id).await;
                            });
                        },
                        if is_en { "Deny" } else { "Refuser" }
                    }

                    button {
                        class: "btn-primary flex-1",
                        onclick: move |_| {
                            let manager = manager_approve.clone();
                            spawn(async move {
                                let _ = manager.approve(request_id).await;
                            });
                        },
                        if is_en { "Approve" } else { "Approuver" }
                    }
                }
            }
        }
    }
}

/// Permission level badge component
#[component]
fn PermissionLevelBadge(level: PermissionLevel) -> Element {
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";
    let (label, bg_style) = match level {
        PermissionLevel::ReadOnly => (
            if is_en { "Read only" } else { "Lecture seule" },
            "background: rgba(52,211,153,0.10); color: #34d399; border: 1px solid rgba(52,211,153,0.20);"
        ),
        PermissionLevel::WriteFile => (
            if is_en { "File write" } else { "Ecriture fichier" },
            "background: rgba(251,191,36,0.10); color: #fbbf24; border: 1px solid rgba(251,191,36,0.20);"
        ),
        PermissionLevel::ReadWrite => (
            if is_en { "Read/Write" } else { "Lecture/Ecriture" },
            "background: rgba(251,191,36,0.10); color: #fbbf24; border: 1px solid rgba(251,191,36,0.20);"
        ),
        PermissionLevel::ExecuteSafe => (
            if is_en { "Safe commands" } else { "Commandes sures" },
            "background: rgba(251,146,60,0.10); color: #fb923c; border: 1px solid rgba(251,146,60,0.20);"
        ),
        PermissionLevel::ExecuteUnsafe => (
            if is_en { "Unsafe commands" } else { "Commandes dangereuses" },
            "background: rgba(248,113,113,0.10); color: #f87171; border: 1px solid rgba(248,113,113,0.20);"
        ),
        PermissionLevel::Network => (
            if is_en { "Network" } else { "Reseau" },
            "background: rgba(56,189,248,0.10); color: #38bdf8; border: 1px solid rgba(56,189,248,0.20);"
        ),
    };

    rsx! {
        span {
            class: "px-2 py-1 rounded-md text-xs font-medium",
            style: "{bg_style}",
            "{label}"
        }
    }
}
