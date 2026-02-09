# LocalClaw Knowledge Base (AGENTS.md)

## OVERVIEW
Native desktop application for local LLM inference and agentic workflows.
Stack: Rust, Dioxus (WebView UI), llama.cpp (inference), Tokio (async runtime).
Features: 30+ agentic tools, GPU acceleration (CUDA/Vulkan), MCP support.
Philosophy: 100% private, local-first, agent-centric.

## STRUCTURE
- `src/main.rs`: Application entry point. Tracing init, storage setup, window launch.
- `src/lib.rs`: Library root. Module declarations and shared utilities.
- `src/app.rs`: Root UI component & `AppState` (Signals/Context).
- `src/agent/`: AI coordinator, state machine, permissions, tools.
- `src/inference/`: llama.cpp integration on dedicated OS thread.
- `src/storage/`: Persistence (JSON) for settings, history, model management.
- `src/system/`: Hardware detection (VRAM/GPU/Resources).
- `src/types/`: Shared domain types (Message, Config, Model).
- `src/ui/`: Dioxus component tree (Chat, Sidebar, Settings).
- `assets/`: UI assets (CSS glassmorphism, icons).

## WHERE TO LOOK
- **Core Loop**: `src/agent/loop_runner.rs` (State machine: Thinking -> Acting -> Observing).
- **LLM Ops**: `src/inference/engine.rs` (Model loading, token streaming).
- **Tool Logic**: `src/agent/tools.rs` (`Tool` trait, registry) and `src/agent/runner.rs`.
- **Tool Implementation**: `src/agent/tools/` (Individual .rs files per domain).
- **Permissions**: `src/agent/permissions.rs` (Approval workflow and levels).
- **Planning**: `src/agent/planning.rs` (Task decomposition and TODO system).
- **Global State**: `src/app.rs` (`AppState` struct).
- **UI Rendering**: `src/ui/chat/message.rs` (Markdown, code blocks, tool calls).
- **UI Styling**: `assets/styles.css` (Glassmorphism, themes, CSS variables).

## CODE MAP
- `src/agent/mod.rs`: Tool registration, Agent struct, and configuration.
- `src/agent/loop_runner.rs`: 9-state machine with loop detection and retry logic.
- `src/agent/tools.rs`: `Tool` trait definition & `DashMap` registry management.
- `src/agent/permissions.rs`: 6-level permission system (ReadOnly to Network).
- `src/inference/engine.rs`: CRITICAL inference thread management and KV cache.
- `src/inference/streaming.rs`: Token-by-token stream processing.
- `src/storage/mod.rs`: Platform-specific data directory management.
- `src/storage/settings.rs`: JSON-based user preference persistence.
- `src/ui/mod.rs`: Main layout, header model picker, and welcome screen.
- `src/ui/chat/mod.rs`: Main chat interface and message streaming logic.
- `src/ui/sidebar/mod.rs`: Navigation, conversation history, and model management.

## CONVENTIONS
- **Inference Isolation**: All llama.cpp operations run on a dedicated worker thread.
- **!Send Safety**: llama-cpp-2 types are NOT `Send`. NEVER move them across threads.
- **KV Cache Persistence**: `LlamaContext` must be reused between generations for performance.
- **Error Handling**: Use `thiserror` for enums. Return `Result`. Avoid `unwrap()`.
- **Logging**: Use `tracing` crate exclusively. No `println!`.
- **Reactivity**: Use Dioxus `Signal` and `provide_context` for global state.
- **Doc Style**: Module-level docs using `//!`. Document `unsafe` with `// SAFETY:`.
- **Async Traits**: Implement `Tool` using `#[async_trait]`.
- **Async Safety**: Use `tokio::sync::Mutex` for async-aware locking.
- **Serialization**: Use `serde` with `#[serde(default)]` for config robustness.
- **Drop Order**: ALWAYS drop `LlamaContext` before `LlamaModel` to prevent segfaults.

## ANTI-PATTERNS
- **Thread Violation**: Moving llama-cpp handles to other threads (causes immediate crashes).
- **Context Churn**: Recreating `LlamaContext` for every generation (slow, triggers VRAM churn).
- **Unsafe Drops**: Dropping Model before Context (causes use-after-free/segfaults).
- **Direct Console**: Using `println!` or `eprintln!` instead of `tracing` macros.
- **Panic Prone**: Using `unwrap()` or `expect()` in production or fallible paths.
- **Blocking Async**: Running heavy compute or llama.cpp calls in Tokio tasks directly.

## COMMANDS
```bash
# Build
cargo build --release               # CPU + Vulkan auto-detect
cargo build --release --features cuda    # NVIDIA GPU acceleration
cargo build --release --features vulkan  # Explicit Vulkan support

# Run & Test
cargo run --release                 # Start desktop application
cargo test                          # Run all unit and integration tests
cargo check                         # Fast code validation without building
cargo test test_name                # Run specific test case

# Windows Helpers
./build.bat                         # Setup MSVC and build CPU
./build_cuda.bat                    # Setup MSVC and build CUDA
```

## HOTSPOTS
- **`src/ui/chat/message.rs`**: Largest file (1k+ lines), handles complex markdown/code rendering.
- **`src/agent/tools.rs`**: Core tool registry and definition (1k+ lines).
- **`src/ui/chat/mod.rs`**: Complex UI state management for streaming (1k+ lines).
- **`src/inference/engine.rs`**: High-complexity worker thread management.
- **`src/agent/loop_runner.rs`**: 9-state machine with deep branching logic.

## ARCHITECTURE PATTERNS
- **Worker Thread Pattern**: LlamaEngine uses dedicated OS thread for llama-cpp handles.
- **Signal-Based Reactivity**: Dioxus Signals for UI state and cross-component updates.
- **Trait-Based Tools**: Extensible tool system using `Tool` trait and `async_trait`.
- **DashMap Registry**: Thread-safe global tool registry.
- **Permission Hierarchy**: Granular 6-level permission system with user approval flow.
- **Event-Driven**: Agent emits `AgentEvent` for real-time UI updates during loop.
- **Statistics**: 53 Rust files, ~17,162 LOC, 2 complex state machines.
- **Performance**: Dev profile uses `opt-level 2` for usable LLM speed during development.
- **Safety**: Dedicated worker thread is a native OS thread, not a Tokio-managed task.
- **Storage**: JSON-based. Located in `%APPDATA%` (Win), `~/Library` (macOS), or `.local/share` (Linux).
- **Tool Groups**: Filesystem, Shell, Git, Web, Exa, Dev, System, PDF, MCP.
- **Permissions**: Levels 0 (ReadOnly) through 5 (Network). Approval required by default.
- **Iteration Limits**: Agent loop capped at 25 iterations or 5 minutes per request.
- **Model Support**: GGUF format only via `llama.cpp` bindings.
