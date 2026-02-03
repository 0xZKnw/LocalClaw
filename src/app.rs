//! Root Dioxus application component
//!
//! This module contains the main App component that serves as the root of the UI tree.

use dioxus::prelude::*;

/// Root application component
#[component]
pub fn App() -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; font-family: system-ui, -apple-system, sans-serif; background-color: #1a1a2e; color: #eee;",
            h1 {
                style: "font-size: 2.5rem; margin-bottom: 1rem;",
                "LocaLM"
            }
            p {
                style: "color: #888; font-size: 1rem;",
                "Local LLM Chat Application"
            }
            p {
                style: "color: #666; font-size: 0.875rem; margin-top: 2rem;",
                "v0.1.0 - Scaffolding Complete"
            }
        }
    }
}
