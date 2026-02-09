
### UI Module Organization (2026-02-09)
- The UI is organized into feature-specific submodules (chat, sidebar, settings, components).
- AppState is shared via Dioxus context and contains global signals for agent, engine, settings, and conversations.
- A simple `t()` helper function is used for bilingual support (FR/EN) throughout the UI.
- Glassmorphism is implemented via central CSS variables in `assets/styles.css`.
