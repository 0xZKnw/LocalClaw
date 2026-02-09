//! Chat input component - Premium glass style with send button inside

use crate::app::AppState;
use crate::agent::skills::loader::SkillLoader;
use crate::agent::skills::Skill;
use dioxus::prelude::*;

/// Estimate how many rows the textarea needs based on content
fn compute_rows(text: &str) -> usize {
    let newlines = text.chars().filter(|&c| c == '\n').count();
    // Each visual line ~ 70 chars for our input width
    let wrap_lines: usize = text.lines().map(|line| {
        if line.is_empty() { 0 } else { (line.len().saturating_sub(1)) / 70 }
    }).sum();
    let total = newlines + wrap_lines + 1;
    total.clamp(1, 8)
}

#[component]
pub fn ChatInput(
    on_send: EventHandler<String>,
    on_stop: EventHandler<()>,
    is_generating: bool,
) -> Element {
    let mut text = use_signal(|| String::new());
    let mut skills = use_signal(Vec::new);
    let mut filtered_skills = use_signal(Vec::<Skill>::new);
    let mut autocomplete_open = use_signal(|| false);
    let mut selected_index = use_signal(|| 0);
    
    let app_state = use_context::<AppState>();
    let is_en = app_state.settings.read().language == "en";

    // Load skills on mount
    use_effect(move || {
        spawn(async move {
            let loaded = SkillLoader::load_all().await;
            skills.set(loaded);
        });
    });

    let handle_keydown = move |evt: KeyboardEvent| {
        // Autocomplete navigation
        if autocomplete_open() {
            let skills_len = filtered_skills.read().len();
            if skills_len > 0 {
                match evt.key() {
                    Key::ArrowUp => {
                        evt.prevent_default();
                        let idx = selected_index();
                        selected_index.set(if idx == 0 { skills_len - 1 } else { idx - 1 });
                        return;
                    }
                    Key::ArrowDown => {
                        evt.prevent_default();
                        selected_index.set((selected_index() + 1) % skills_len);
                        return;
                    }
                    Key::Enter => {
                        evt.prevent_default();
                        let skill = filtered_skills.read()[selected_index()].clone();
                        let name = skill.name.trim_start_matches("skill_");
                        text.set(format!("/{} ", name));
                        autocomplete_open.set(false);
                        return;
                    }
                    Key::Escape => {
                        evt.prevent_default();
                        autocomplete_open.set(false);
                        return;
                    }
                    _ => {}
                }
            }
        }

        if evt.key() == Key::Escape && is_generating {
            on_stop.call(());
        } else if evt.key() == Key::Enter && !evt.modifiers().contains(Modifiers::SHIFT) {
            evt.prevent_default();
            if !is_generating && !text().trim().is_empty() {
                on_send.call(text());
                text.set(String::new());
                autocomplete_open.set(false);
            }
        }
    };

    let handle_input = move |evt: FormEvent| {
        let val = evt.value();
        text.set(val.clone());

        // Check for autocomplete trigger
        if val.starts_with('/') && !val.contains(' ') && !val.contains('\n') {
            let query = val.trim_start_matches('/');
            let all = skills.read();
            let matches: Vec<Skill> = all.iter()
                .filter(|s| {
                    let name = s.name.trim_start_matches("skill_");
                    name.to_lowercase().contains(&query.to_lowercase())
                })
                .cloned()
                .collect();
            
            if !matches.is_empty() {
                filtered_skills.set(matches);
                selected_index.set(0);
                autocomplete_open.set(true);
            } else {
                autocomplete_open.set(false);
            }
        } else {
            autocomplete_open.set(false);
        }
    };

    let can_send = !is_generating && !text().trim().is_empty();
    let rows = compute_rows(&text());
    let rows_str = format!("{}", rows);
    let is_multiline = rows > 1;

    // Pre-compute all dynamic attribute values to avoid type inference issues in rsx!
    let container_class = if is_multiline {
        "glass-input flex items-end gap-2 pr-2"
    } else {
        "glass-input flex items-center gap-2 pr-2"
    };

    let textarea_style = if is_multiline {
        "line-height: 22px; padding: 14px 0 14px 20px; max-height: 180px; overflow-y: auto;"
    } else {
        "line-height: 22px; padding: 15px 0 15px 20px; max-height: 180px; overflow: hidden;"
    };

    let placeholder = if is_en { "Send a message..." } else { "Envoyer un message..." };

    let stop_style = if is_multiline {
        "background: var(--error); margin-bottom: 8px;"
    } else {
        "background: var(--error);"
    };
    let stop_title = if is_en { "Stop (Esc)" } else { "Arreter (Esc)" };

    let send_class = if can_send {
        "flex-shrink-0 w-9 h-9 rounded-full flex items-center justify-center transition-all hover:scale-105 active:scale-95"
    } else {
        "flex-shrink-0 w-9 h-9 rounded-full flex items-center justify-center transition-all cursor-not-allowed opacity-30"
    };

    let mb = if is_multiline { " margin-bottom: 8px;" } else { "" };
    let send_style = if can_send {
        format!("background: var(--accent-primary); color: #F2EDE7; box-shadow: 0 2px 8px -2px rgba(42,107,124,0.3);{mb}")
    } else {
        format!("background: var(--bg-elevated);{mb}")
    };

    let send_title = if is_en { "Send (Enter)" } else { "Envoyer (Entree)" };
    let hint = if is_en { "Enter to send, Shift+Enter for a new line" } else { "Entree pour envoyer, Shift+Entree pour un saut de ligne" };

    rsx! {
        div {
            class: "w-full px-4 pb-5 pt-2",

            div {
                class: "relative max-w-3xl mx-auto",

                // Autocomplete Dropdown
                if autocomplete_open() {
                    div {
                        class: "absolute left-0 bottom-full mb-2 w-full rounded-xl overflow-hidden z-50 glass-md animate-fade-in-up",
                        style: "max-height: 240px; border: 1px solid var(--border-medium); box-shadow: 0 12px 32px -4px rgba(30,25,20,0.35);",
                        
                        // Header
                        div {
                            class: "px-3 py-2 border-b border-[var(--border-subtle)] bg-white/5",
                            span {
                                class: "text-[10px] uppercase tracking-widest text-[var(--text-tertiary)] font-semibold",
                                if is_en { "Available Skills" } else { "Skills disponibles" }
                            }
                        }
                        
                        // List
                        div {
                            class: "overflow-y-auto custom-scrollbar",
                            style: "max-height: 200px;",
                            
                            for (i, skill) in filtered_skills.read().iter().enumerate() {
                                {
                                    let is_selected = i == selected_index();
                                    let name = skill.name.trim_start_matches("skill_").to_string();
                                    let desc = if skill.description.len() > 60 {
                                        format!("{}...", &skill.description[..60])
                                    } else {
                                        skill.description.clone()
                                    };
                                    
                                    rsx! {
                                        button {
                                            onclick: move |_| {
                                                text.set(format!("/{} ", name));
                                                autocomplete_open.set(false);
                                            },
                                            class: "w-full text-left px-3 py-2 transition-colors flex flex-col gap-0.5",
                                            style: if is_selected {
                                                "background: var(--accent-soft); color: var(--accent-primary);"
                                            } else {
                                                "color: var(--text-primary); hover:bg-white/5;"
                                            },
                                            
                                            div {
                                                class: "flex items-center justify-between",
                                                span { class: "font-semibold text-sm", "/{name}" }
                                            }
                                            span {
                                                class: "text-xs opacity-70 truncate",
                                                "{desc}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Glass input container
                div {
                    class: "{container_class}",
                    style: "border-radius: 28px; min-height: 52px;",

                    // Textarea — auto-expanding
                    textarea {
                        class: "flex-1 bg-transparent outline-none text-[var(--text-primary)] resize-none placeholder-[var(--text-tertiary)] text-[15px] custom-scrollbar",
                        style: "{textarea_style}",
                        placeholder: "{placeholder}",
                        value: "{text}",
                        oninput: handle_input,
                        onkeydown: handle_keydown,
                        disabled: is_generating,
                        rows: "{rows_str}",
                    }

                    // Send / Stop button
                    if is_generating {
                        button {
                            onclick: move |_| on_stop.call(()),
                            class: "flex-shrink-0 w-9 h-9 rounded-full flex items-center justify-center text-white transition-all animate-pulse-ring",
                            style: "{stop_style}",
                            title: "{stop_title}",
                            svg {
                                width: "14",
                                height: "14",
                                view_box: "0 0 24 24",
                                fill: "currentColor",
                                rect { x: "6", y: "6", width: "12", height: "12", rx: "2" }
                            }
                        }
                    } else {
                        button {
                            onclick: move |_| {
                                if can_send {
                                    on_send.call(text());
                                    text.set(String::new());
                                }
                            },
                            disabled: !can_send,
                            class: "{send_class}",
                            style: "{send_style}",
                            title: "{send_title}",
                            svg {
                                width: "16",
                                height: "16",
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
                }

                // Hint text
                p {
                    class: "text-center text-[11px] text-[var(--text-tertiary)] mt-2 opacity-40",
                    "{hint}"
                }
            }
        }
    }
}
