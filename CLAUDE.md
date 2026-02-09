# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

LocaLM is a native desktop application for running LLMs locally without cloud dependencies. Built in Rust with Dioxus (WebView-based desktop UI) and llama.cpp for inference. Supports 30+ agentic tools, GPU acceleration (CUDA/Vulkan), and bilingual UI (French/English).

## Build & Run Commands

```bash
# Build (CPU only)
cargo build --release

# Build with GPU support
cargo build --release --features cuda
cargo build --release --features vulkan

# Run
cargo run --release

# Run tests
cargo test

# Run a single test
cargo test test_name

# Check without building
cargo check

# Windows: use build.bat or build_cuda.bat for VC++ environment setup
```

**Build prerequisites:** Rust 1.75+, CMake, C++ compiler (MSVC on Windows, GCC/Clang on Linux/macOS).

**Dev profile uses opt-level 2** so llama.cpp runs at reasonable speed even without `--release`.

## Architecture

### Core Data Flow

User message → `AppState` (app.rs) → Agent loop (loop_runner.rs) → LLM inference (engine.rs) → Tool execution → streamed response back to UI.

### Key Modules

- **`src/main.rs`** — Entry point. Initializes tracing, storage, launches Dioxus desktop window.
- **`src/app.rs`** — Root component. Creates `AppState` (holds Agent, LlamaEngine, settings, conversations as Dioxus Signals). Provides state via context.
- **`src/agent/`** — The agentic AI system, the most complex module:
  - `loop_runner.rs` — State machine: Analyzing → Planning → Thinking → Acting → Observing → Reflecting → Responding → Completed. Has infinite loop detection, retry with backoff, iteration limits (default 25), runtime cap (5 min).
  - `tools.rs` — `Tool` trait (`name`, `description`, `parameters_schema`, `execute`) + `ToolRegistry` using `DashMap<String, Arc<dyn Tool>>`.
  - `permissions.rs` — Hierarchical permission levels: ReadOnly(0) → WriteFile(1) → ReadWrite(2) → ExecuteSafe(3) → ExecuteUnsafe(4) → Network(5). Allowlist + auto-approve modes.
  - `runner.rs` — Parses tool calls from LLM text output, formats results back.
  - `prompts.rs` — Dynamic system prompt generation (injects OS info, language, available tools).
  - `planning.rs` — Optional TODO-style task planning for multi-step requests.
- **`src/agent/tools/`** — 30+ tool implementations organized by domain (filesystem.rs, shell.rs, git.rs, web.rs, exa.rs, dev.rs, system.rs, pdf.rs, mcp_client.rs).
- **`src/inference/`** — llama.cpp integration:
  - `engine.rs` — **Critical design:** All inference runs on a dedicated worker thread (llama-cpp-2 types are not `Send`), communication via channels. KV cache is persisted between generations (not recreated) for performance.
  - `streaming.rs` — Token-by-token streaming.
  - `model.rs` — GGUF format validation.
- **`src/storage/`** — JSON file-based persistence: settings.json, conversations/*.json, model scanning, HuggingFace downloads.
- **`src/system/`** — GPU/VRAM detection, RAM/CPU monitoring.
- **`src/ui/`** — Dioxus components: Layout → Header + Sidebar + ChatView/SettingsPanel. Uses Signal-based reactivity and async spawns.
- **`src/types/`** — Shared types for config, messages, and model metadata.

### Adding a New Tool

1. Implement the `Tool` trait (async_trait) in a file under `src/agent/tools/`.
2. Register it in `src/agent/mod.rs` during agent initialization.
3. Assign an appropriate permission level in the registration.

### Inference Thread Isolation

The `LlamaEngine` uses a dedicated OS thread (not a Tokio task) because llama-cpp-2 types contain raw pointers that are `!Send`. The async API communicates with this thread via `std::sync::mpsc` channels. Do not attempt to move llama-cpp types across thread boundaries.

## Storage Locations

Data is stored in platform-specific directories via the `directories` crate:
- **Windows:** `%APPDATA%\LocaLM\LocaLM\`
- **macOS:** `~/Library/Application Support/com.LocaLM.LocaLM/`
- **Linux:** `~/.local/share/LocaLM/`

Subdirectories: `conversations/`, `models/`, `settings.json`.

## Feature Flags

- `cuda` — Enables NVIDIA CUDA GPU acceleration (requires CUDA Toolkit).
- `vulkan` — Enables Vulkan GPU acceleration.
- Default (no features) — CPU only with Vulkan auto-detect.
