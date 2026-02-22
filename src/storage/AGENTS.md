# Storage Module (src/storage/)

## OVERVIEW
JSON-based persistence layer for LocalClaw. Handles settings, conversations, model metadata, and HuggingFace downloads. Platform-aware data directory selection.

## STRUCTURE
- `mod.rs`: Data directory resolution (platform-specific).
- `settings.rs`: User preferences (JSON), validation, defaults.
- `conversations.rs`: Chat history, message serialization, title generation.
- `models.rs`: GGUF model scanning, metadata extraction, size formatting.
- `huggingface.rs`: Model download from HuggingFace Hub.

## KEY TYPES
- `Settings`: User config with serde defaults (inference params, UI preferences, permissions).
- `Conversation`: Chat history container with messages, timestamps, metadata.
- `ModelInfo`: GGUF file metadata (path, size, param count if parseable).
- `StorageError`: Error enum for file I/O, JSON parsing, network failures.

## CONVENTIONS
- **Platform Paths**: Use `directories` crate for OS-aware data dirs.
- **JSON Persistence**: serde_json for all storage files.
- **Lazy Load**: Don't load all history on startup; paginate if needed.
- **Error Handling**: Return `Result` for all I/O operations.

## TESTING
- Uses `tempfile` crate for isolated filesystem tests.
- 14 unit tests across 4 modules.
- Binary file tests construct headers manually (GGUF magic bytes).
