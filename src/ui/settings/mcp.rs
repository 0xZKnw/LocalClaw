#![allow(non_snake_case)]

use crate::agent::mcp_config;
use crate::agent::skills::loader::SkillLoader;
use crate::app::AppState;
use crate::storage::settings::save_settings;
use crate::storage::get_data_dir;
use dioxus::prelude::*;

pub fn McpSettings() -> Element {
    let app_state = use_context::<AppState>();
    let settings = app_state.settings.read().clone();
    let is_en = settings.language == "en";
    let disabled_servers = settings.disabled_mcp_servers.clone();

    // Load MCP servers
    let mcp_servers = use_resource(|| async {
        mcp_config::load_effective_config().await
    });

    // Load Skills
    let skills = use_resource(|| async {
        SkillLoader::load_all().await
    });

    let mut app_state_toggle = app_state.clone();

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in-up pb-8",
            
            // Header with Open Config button
            div {
                class: "flex items-center justify-between",
                h2 {
                    class: "text-lg font-semibold text-[var(--text-primary)]",
                    if is_en { "MCP Configuration" } else { "Configuration MCP" }
                }
                button {
                    class: "px-3 py-1.5 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] text-sm text-[var(--text-secondary)] transition-colors border border-[var(--border-subtle)]",
                    onclick: move |_| {
                        spawn(async move {
                            if let Ok(data_dir) = get_data_dir() {
                                let path = data_dir.join("mcp.json");
                                // Ensure file exists before opening (create empty if needed)
                                if !path.exists() {
                                    let _ = tokio::fs::write(&path, r#"{ "mcpServers": {} }"#).await;
                                }
                                
                                #[cfg(target_os = "windows")]
                                let _ = std::process::Command::new("explorer").arg(path).spawn();
                                #[cfg(target_os = "macos")]
                                let _ = std::process::Command::new("open").arg(path).spawn();
                                #[cfg(target_os = "linux")]
                                let _ = std::process::Command::new("xdg-open").arg(path).spawn();
                            }
                        });
                    },
                    if is_en { "Edit mcp.json" } else { "Editer mcp.json" }
                }
            }

            // MCP Servers List
            div { class: "p-5 rounded-2xl glass-md",
                h3 { 
                    class: "text-base font-semibold mb-4 text-[var(--text-primary)]",
                    if is_en { "MCP Servers" } else { "Serveurs MCP" }
                }

                if let Some(servers) = mcp_servers.read().as_ref() {
                    if servers.is_empty() {
                        div { 
                            class: "text-sm text-[var(--text-tertiary)] italic",
                            if is_en { "No MCP servers configured." } else { "Aucun serveur MCP configure." }
                        }
                    } else {
                        div { class: "space-y-3",
                            for server in servers {
                                {
                                    let server_id = server.id.clone();
                                    let is_enabled = !disabled_servers.contains(&server_id);
                                    let transport_info = match &server.transport {
                                        crate::agent::McpTransport::Stdio { command, args: _ } => format!("stdio: {}", command),
                                        crate::agent::McpTransport::Http { url } => format!("http: {}", url),
                                    };
                                    
                                    rsx! {
                                        div {
                                            class: "flex items-center justify-between p-3 rounded-xl border border-[var(--border-subtle)] bg-white/[0.01]",
                                            
                                            div {
                                                div { class: "font-medium text-[var(--text-primary)]", "{server.name}" }
                                                div { class: "text-xs text-[var(--text-tertiary)] font-mono mt-0.5", "{transport_info}" }
                                            }

                                            button {
                                                onclick: {
                                                    let server_id = server_id.clone();
                                                    move |_| {
                                                        let mut settings = app_state_toggle.settings.write();
                                                        if is_enabled {
                                                            settings.disabled_mcp_servers.push(server_id.clone());
                                                        } else {
                                                            settings.disabled_mcp_servers.retain(|id| id != &server_id);
                                                        }
                                                        if let Err(e) = save_settings(&settings) {
                                                            tracing::error!("Failed to save settings: {}", e);
                                                        }
                                                    }
                                                },
                                                class: if is_enabled { "toggle-switch active" } else { "toggle-switch" },
                                                div { class: "toggle-switch-knob" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "animate-pulse h-20 bg-white/[0.02] rounded-xl" }
                }
            }
            
            // Skills List
            div { class: "p-5 rounded-2xl glass-md",
                h3 { 
                    class: "text-base font-semibold mb-4 text-[var(--text-primary)]",
                    if is_en { "Skills" } else { "Comptences (Skills)" }
                }

                if let Some(loaded_skills) = skills.read().as_ref() {
                    if loaded_skills.is_empty() {
                        div { 
                            class: "text-sm text-[var(--text-tertiary)] italic",
                            if is_en { "No skills loaded." } else { "Aucune competence chargee." }
                        }
                    } else {
                        div { class: "grid grid-cols-1 md:grid-cols-2 gap-3",
                            for skill in loaded_skills {
                                div {
                                    class: "p-3 rounded-xl border border-[var(--border-subtle)] bg-white/[0.01]",
                                    div { class: "font-medium text-[var(--text-primary)] mb-1", "{skill.name}" }
                                    div { class: "text-xs text-[var(--text-tertiary)] line-clamp-2", "{skill.description}" }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "animate-pulse h-20 bg-white/[0.02] rounded-xl" }
                }
            }
        }
    }
}
