use crate::agent::skills::loader::SkillLoader;
use crate::app::AppState;
use dioxus::prelude::*;

pub fn SkillsSettings() -> Element {
    let app_state = use_context::<AppState>();
    
    // Use resource to load skills async
    let mut skills_resource = use_resource(move || async move {
        SkillLoader::load_all().await
    });

    let app_state_delete = app_state.clone();

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in-up pb-8",

            // Header & Add Button
            div {
                class: "flex items-center justify-between",
                h2 {
                    class: "text-lg font-semibold text-[var(--text-primary)]",
                    { "Skills Manager" }
                }

                button {
                    class: "px-4 py-2 bg-[var(--accent-primary)] hover:bg-[var(--accent-hover)] text-white rounded-lg text-sm font-medium transition-colors flex items-center gap-2",
                    onclick: move |_| {
                         // Redirect logic (placeholder)
                    },
                    span { "+" }
                    { "New Skill" }
                }
            }

            // Skills List
            {
                let skills = skills_resource.read_unchecked();
                match &*skills {
                    Some(skills) if skills.is_empty() => rsx! {
                        div {
                            class: "p-8 text-center text-[var(--text-tertiary)] border border-dashed border-[var(--border-medium)] rounded-xl",
                            "No skills installed yet."
                        }
                    },
                    Some(skills) => rsx! {
                        div {
                            class: "grid gap-4",
                            for skill in skills {
                                div {
                                    class: "p-4 rounded-xl glass-md border border-[var(--border-subtle)] hover:border-[var(--border-medium)] transition-all",
                                    
                                    div {
                                        class: "flex items-start justify-between",
                                        div {
                                            h3 { class: "font-mono text-sm font-semibold text-[var(--text-primary)]", "{skill.name}" }
                                            p { class: "text-sm text-[var(--text-secondary)] mt-1", "{skill.description}" }
                                            div {
                                                class: "flex items-center gap-2 mt-3 text-xs text-[var(--text-tertiary)]",
                                                span { "📂" }
                                                span { class: "font-mono opacity-70", "{skill.path.display()}" }
                                            }
                                        }

                                        button {
                                            class: "p-2 text-[var(--text-tertiary)] hover:text-[#C45B5B] hover:bg-[#C45B5B]/10 rounded-lg transition-colors",
                                            title: "Delete Skill",
                                            onclick: {
                                                let skill_name = skill.name.clone();
                                                let skill_path = skill.path.clone();
                                                let app_state = app_state_delete.clone();
                                                move |_| {
                                                    let name = skill_name.clone();
                                                    let path = skill_path.clone();
                                                    let app_state = app_state.clone();
                                                    
                                                    spawn(async move {
                                                        tracing::info!("Deleting skill: {}", name);
                                                        app_state.agent.tool_registry.remove(&name);
                                                        app_state.agent.skill_registry.remove(&name);
                                                        if let Some(parent) = path.parent() {
                                                            let _ = tokio::fs::remove_dir_all(parent).await;
                                                        }
                                                        skills_resource.restart();
                                                    });
                                                }
                                            },
                                            svg {
                                                class: "w-4 h-4",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                stroke_linecap: "round",
                                                stroke_linejoin: "round",
                                                polyline { points: "3 6 5 6 21 6" }
                                                path { d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2-2v2" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    None => rsx! {
                        div {
                            class: "flex items-center justify-center p-12",
                            div { class: "w-6 h-6 border-2 border-[var(--text-tertiary)] border-t-transparent rounded-full animate-spin" }
                        }
                    }
                }
            }
        }
    }
}
