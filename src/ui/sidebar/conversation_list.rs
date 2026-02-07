use dioxus::prelude::*;

use crate::app::AppState;
use crate::storage::conversations::{
    delete_conversation, list_conversations, save_conversation, Conversation,
};

#[component]
pub fn ConversationList() -> Element {
    let app_state = use_context::<AppState>();

    {
        let mut app_state = app_state.clone();
        use_effect(move || match list_conversations() {
            Ok(conversations) => app_state.conversations.set(conversations),
            Err(e) => tracing::error!("Failed to load conversations: {}", e),
        });
    }

    let _handle_new = {
        let mut conversations_signal = app_state.conversations.clone();
        let mut current_conversation_signal = app_state.current_conversation.clone();
        move |_: ()| {
            let conversation = Conversation::new(None);
            if let Err(e) = save_conversation(&conversation) {
                tracing::error!("Failed to save conversation: {}", e);
                return;
            }
            current_conversation_signal.set(Some(conversation));
            if let Ok(conversations) = list_conversations() {
                conversations_signal.set(conversations);
            }
        }
    };

    let conversations = app_state.conversations.read().clone();
    let selected_id = app_state
        .current_conversation
        .read()
        .as_ref()
        .map(|conv| conv.id.clone());

    rsx! {
        div {
            class: "flex-1 overflow-y-auto p-2 space-y-1 scrollbar-thin",

            if conversations.is_empty() {
                div {
                    class: "flex flex-col items-center justify-center py-10 text-[var(--text-tertiary)] gap-2 opacity-50",
                    svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.5", stroke_dasharray: "4 4", circle { cx: "12", cy: "12", r: "10" } }
                    span { class: "text-xs font-medium", "No recent chats" }
                }
            } else {
                div {
                    class: "text-[10px] uppercase tracking-widest text-[var(--text-tertiary)] font-semibold px-3 py-2 select-none opacity-60",
                    "Recent"
                }

                {conversations.into_iter().map(|conversation| {
                    let is_selected = selected_id
                        .as_ref()
                        .map(|id| id == &conversation.id)
                        .unwrap_or(false);

                    let row_class = if is_selected {
                        "group flex items-center gap-2.5 px-3 py-2 rounded-lg bg-white/[0.08] border-l-2 border-[var(--accent-primary)] text-[var(--text-primary)] cursor-pointer transition-all"
                    } else {
                        "group flex items-center gap-2.5 px-3 py-2 rounded-lg hover:bg-white/[0.05] border-l-2 border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer transition-all"
                    };

                    let conversation_for_select = conversation.clone();
                    let conversation_id = conversation.id.clone();
                    let mut current_conversation_signal = app_state.current_conversation.clone();
                    let mut conversations_signal = app_state.conversations.clone();

                    rsx! {
                        div {
                            key: "{conversation.id}",
                            class: "px-1",
                            onclick: move |_| {
                                current_conversation_signal.set(Some(conversation_for_select.clone()));
                            },

                            div {
                                class: row_class,
                                // Icon
                                div {
                                    class: "shrink-0 " .to_string() + if is_selected { "text-[var(--accent-primary)]" } else { "text-[var(--text-tertiary)] group-hover:text-[var(--text-secondary)]" },
                                    svg {
                                        width: "14",
                                        height: "14",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                                    }
                                }

                                // Title
                                div {
                                    class: "truncate flex-1 text-sm",
                                    "{conversation.title}"
                                }

                                button {
                                    class: "opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded-md hover:bg-white/[0.08] text-[var(--text-tertiary)] hover:text-[var(--text-error)]",
                                    title: if app_state.settings.read().language == "en" { "Delete conversation" } else { "Supprimer la conversation" },
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        if let Err(e) = delete_conversation(&conversation_id) {
                                            tracing::error!("Failed to delete conversation: {}", e);
                                        }
                                        let should_clear = current_conversation_signal
                                            .read()
                                            .as_ref()
                                            .map(|conv| conv.id == conversation_id)
                                            .unwrap_or(false);
                                        if should_clear {
                                            current_conversation_signal.set(None);
                                        }
                                        if let Ok(conversations) = list_conversations() {
                                            conversations_signal.set(conversations);
                                        }
                                    },
                                    svg {
                                        width: "12",
                                        height: "12",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        line { x1: "18", y1: "6", x2: "6", y2: "18" }
                                        line { x1: "6", y1: "6", x2: "18", y2: "18" }
                                    }
                                }
                            }
                        }
                    }
                })}
            }
        }
    }
}
