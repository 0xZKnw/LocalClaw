# UI Module (src/ui/)

## OVERVIEW
- Dioxus-based native desktop UI (WebView).
- Reactive state management using Signals and Context.
- Glassmorphism design system via custom CSS.
- Bilingual interface (FR/EN) support.

## STRUCTURE
- `mod.rs`: Main layout (Sidebar + MainView), `HeaderModelPicker`, i18n helpers.
- `chat/`: Chat interface, message streaming, markdown rendering.
- `sidebar/`: Navigation, conversation history, model selector.
- `settings/`: Multi-tab configuration (Inference, Hardware, Tools, UI, Skills, MCP).
- `components/`: Reusable UI elements (PermissionDialog, Spinners, Monitoring, ToolUsage).

## KEY PATTERNS

### AppState Context
Global state injected at root (`app.rs`) and consumed by components:
```rust
// Provider (src/app.rs)
use_context_provider(|| AppState::new());

// Consumer
let app_state = use_context::<AppState>();
```

### Signal-Based Reactivity
State updates via `Signal<T>` to trigger targeted re-renders:
```rust
// Reading state
let settings = app_state.settings.read().clone();

// Local UI state
let mut dropdown_open = use_signal(|| false);
```

### Internationalization (i18n)
Bilingual support via the `t()` helper in `src/ui/mod.rs`:
```rust
// Returns string based on app_state.settings.language
t(&app_state, "Chargement...", "Loading...")
```

### Component Organization
- **Feature Modules**: Subdirectories (e.g., `chat/`) encapsulate feature-specific logic.
- **Shared Components**: `src/ui/components/` for cross-feature UI blocks.
- **Styling**: Scoped via `assets/styles.css` using BEM-like class naming.

## COMPONENTS
- `Layout`: Top-level container managing Sidebar and View switching.
- `ChatView`: Core interaction surface; manages message list and `ChatInput`.
- `MessageBubble`: Renders Markdown, code blocks, and tool execution status.
- `PermissionDialog`: Critical security gate for tool call approval.
- `HeaderModelPicker`: Fast model switching with VRAM-aware progress bars.
- `Sidebar`: Collapsible navigation and conversation history management.
