# LocalClaw Inference Module (AGENTS.md)

## OVERVIEW
Core LLM engine wrapping `llama-cpp-2`. Handles model loading, context management, and token streaming. 

## CRITICAL CONSTRAINTS
### 1. Thread Isolation (!Send Safety)
- **Constraint**: `llama-cpp-2` types (`LlamaBackend`, `LlamaModel`, `LlamaContext`) contain raw pointers and are NOT `Send`.
- **Rule**: NEVER move these types across thread boundaries. Moving them causes immediate crashes or undefined behavior.
- **Implementation**: All inference logic is pinned to a dedicated OS worker thread (not a Tokio task) spawned in `engine.rs`.
- **Communication**: Interaction with the engine occurs exclusively via `std::sync::mpsc` channels.

### 2. KV Cache Persistence
- **Performance**: Creating a `LlamaContext` takes 2-5 seconds and triggers VRAM allocation.
- **Rule**: PERSIST the `LlamaContext` between generations. 
- **Pattern**: Clear the KV cache using `llama_batch` clearing logic instead of dropping the context.
- **Efficiency**: Reuse allows near-instant response for subsequent chat turns.

### 3. Resource Drop Order
- **Safety**: Manual memory management requires strict drop ordering.
- **Rule**: ALWAYS drop `LlamaContext` before `LlamaModel`.
- **Code Pattern**:
  ```rust
  state.ctx = None;    // Clear context first
  state.model = None;  // Clear model second
  ```

## STRUCTURE
- `src/inference/engine.rs`: Main engine logic, worker thread loop, and channel handling.
- `src/inference/model.rs`: GGUF validation, magic byte checking, and metadata parsing.
- `src/inference/streaming.rs`: Token-by-token streaming implementation and sampler logic.
- `src/inference/mod.rs`: Public module exports and error type mappings.

## KEY TYPES
- `LlamaEngine`: The public, thread-safe handle used by the rest of the application.
- `WorkerState`: Internal state struct living on the worker thread, holding `!Send` handles.
- `GenerationParams`: Inference configuration (temperature, top_p, context size).
- `EngineError`: Error enumeration using `thiserror` for precise failure reporting.
- `LoadedModelInfo`: Metadata about the currently active model (vram usage, param count).

## PATTERNS
- **Inference Isolation**: The engine uses an OS thread to prevent blocking the Tokio runtime.
- **VRAM Awareness**: Model loading logic should verify VRAM availability before allocation.
- **Error Propagation**: Use `?` operator to bubble up inference errors to the UI layer.
- **Atomic Cancellation**: Generation can be interrupted via `AtomicBool` flags checked in the loop.
- **Streaming**: Token-by-token output via mpsc channel to UI layer.
