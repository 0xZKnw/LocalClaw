<p align="center">
  <h1 align="center">LocalClaw</h1>
  <p align="center">
    <strong>Your private AI, 100% local.</strong><br>
    A premium desktop application to run LLMs locally with an agentic tool system, built in Rust.
  </p>
  <p align="center">
    <img src="https://img.shields.io/badge/rust-2021-orange?style=flat-square&logo=rust" alt="Rust 2021">
    <img src="https://img.shields.io/badge/version-0.2.0-blue?style=flat-square" alt="Version 0.2.0">
    <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License MIT">
    <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=flat-square" alt="Platform">
  </p>
</p>

---

## What is LocalClaw?

LocalClaw is a native desktop application that lets you run large language models **entirely on your machine** — no cloud, no API keys, no data leaving your device. It combines a modern glassmorphism UI with a powerful agentic system that can read/write files, execute commands, search the web, and much more.

Think of it as your own private Claude or ChatGPT, running offline with full access to your computer.

### Key Features

- **100% Local & Private** — All inference runs on your hardware via `llama.cpp`. Your data never leaves your machine.
- **Agentic Tool System** — 30+ built-in tools: file operations, shell execution, git, web search, code search, and more.
- **GGUF Model Support** — Load any `.gguf` model. Download directly from HuggingFace within the app.
- **GPU Acceleration** — Optional CUDA and Vulkan support for fast inference.
- **Premium UI** — Warm organic design with glassmorphism, dark/light themes, smooth animations.
- **Bilingual** — Full French and English interface.
- **Permission System** — Granular tool permissions with allowlist and auto-approve mode.
- **MCP Protocol** — Connect to external Model Context Protocol servers for extended capabilities.
- **Conversation History** — Persistent chat history saved locally.
- **VRAM-Aware** — Automatically caps context size based on your available VRAM.

---

## Screenshots

> *Coming soon*

---

## Installation

### Prerequisites

- **Rust** (1.75+ recommended) — [rustup.rs](https://rustup.rs)
- **CMake** — Required to build `llama.cpp`
- **C++ Compiler** — MSVC on Windows, GCC/Clang on Linux/macOS

#### Windows

Visual Studio Build Tools with C++ workload are required. The included `build.bat` handles the setup:

```bash
# Standard build (CPU + Vulkan auto-detect)
cargo build --release

# With CUDA support (requires CUDA Toolkit)
cargo build --release --features cuda

# With Vulkan support
cargo build --release --features vulkan
```

#### Linux / macOS

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install cmake build-essential libssl-dev pkg-config

# Build
cargo build --release
```

---

## Quick Start

1. **Build the project:**
   ```bash
   cargo build --release
   ```

2. **Run the app:**
   ```bash
   cargo run --release
   ```

3. **Load a model:**
   - Place any `.gguf` model file in the models directory, or
   - Use the built-in HuggingFace downloader (sidebar > "Download from HuggingFace")
   - Select and load a model from the header dropdown or sidebar

4. **Start chatting!**
   The AI can read your files, run commands, search the web, and more — all locally.

---

## Architecture

```
src/
├── main.rs              # Entry point, window setup
├── app.rs               # Application state (AppState)
├── agent/               # Agentic AI system
│   ├── mod.rs           # Agent config, tool registration
│   ├── permissions.rs   # Permission levels & approval workflow
│   ├── prompts.rs       # Dynamic system prompts
│   ├── planning.rs      # Task planning (TODO system)
│   ├── runner.rs        # Tool call extraction & formatting
│   ├── loop_runner.rs   # Agent loop (think → act → observe)
│   └── tools/           # 30+ tools
│       ├── filesystem.rs    # File read/write/edit/search
│       ├── shell.rs         # Bash/PowerShell execution
│       ├── git.rs           # Git operations
│       ├── web.rs           # Web fetch/download
│       ├── exa.rs           # Exa web & code search
│       ├── dev.rs           # Diff, find-replace, patch
│       ├── system.rs        # Process list, sysinfo, tree
│       └── mcp_client.rs    # MCP protocol client
├── inference/           # LLM engine (llama.cpp bindings)
│   ├── engine.rs        # Model loading, generation
│   └── streaming.rs     # Token-by-token streaming
├── storage/             # Persistence layer
│   ├── settings.rs      # User preferences (JSON)
│   ├── conversations.rs # Chat history
│   ├── models.rs        # GGUF model scanning
│   └── huggingface.rs   # Model downloading
├── system/              # Hardware detection
│   ├── gpu.rs           # GPU/VRAM detection
│   └── resources.rs     # RAM/CPU monitoring
└── ui/                  # Dioxus UI components
    ├── mod.rs           # Layout, header model picker
    ├── chat/            # Chat view, messages, input
    ├── sidebar/         # Sidebar, model picker, conversations
    ├── settings/        # Settings tabs (inference, hardware, tools, appearance)
    └── components/      # Permission dialog, loading spinners
```

---

## Tools

LocalClaw comes with **30+ built-in tools** the AI can use:

| Category | Tools | Permission |
|----------|-------|------------|
| **File Read** | `file_read`, `file_list`, `grep`, `glob`, `file_info`, `file_search` | Read only |
| **File Write** | `file_write`, `file_edit`, `file_create`, `file_delete`, `file_move`, `file_copy`, `directory_create` | Write |
| **Shell** | `bash`, `bash_background`, `command` | Execute |
| **Git** | `git_status`, `git_diff`, `git_log`, `git_commit`, `git_branch`, `git_stash` | Read / Execute |
| **Web** | `web_search`, `code_search`, `company_research`, `web_fetch`, `web_download` | Network |
| **Dev** | `diff`, `find_replace`, `patch`, `wc` | Read / Write |
| **System** | `process_list`, `environment`, `system_info`, `which`, `tree` | Read only |

### Permission Modes

- **Manual approval** (default) — Each tool call shows a permission dialog
- **Allowlist** — Pre-approve specific tools or tool groups in Settings > Tools
- **Auto-approve all** — Skip all permission dialogs (use with caution)

---

## Settings

Accessible via the sidebar gear icon:

| Tab | Options |
|-----|---------|
| **Inference** | Temperature, Top-p, Top-k, Max tokens, Context size, System prompt |
| **Hardware** | GPU layers, VRAM monitoring, Models directory, Auto-load model |
| **Tools** | Auto-approve mode, Tool allowlist (per-group and per-tool) |
| **Appearance** | Dark/Light theme, Font size, Language (FR/EN) |

---

## Tech Stack

- **Language:** Rust (2021 edition)
- **UI Framework:** [Dioxus](https://dioxuslabs.com) (native desktop via WebView)
- **LLM Backend:** [llama.cpp](https://github.com/ggerganov/llama.cpp) via `llama-cpp-2` crate
- **Async Runtime:** Tokio
- **Styling:** Custom CSS with glassmorphism, CSS variables for theming

---

## Contributing

Contributions are welcome! Feel free to open issues or pull requests.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <sub>Built with Rust and love. Your AI, your rules, your machine.</sub>
</p>
