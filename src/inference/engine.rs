//! Inference engine implementation
//!
//! Core logic for managing llama-cpp context and running inference.
//!
//! # Architecture
//!
//! Since llama-cpp-2 types (`LlamaBackend`, `LlamaModel`, `LlamaContext`) contain
//! raw pointers that are not `Send`, all inference operations run on a dedicated
//! worker thread. The main thread communicates via channels.
//!
//! # Performance (Critical)
//!
//! The LlamaContext (KV cache) is PERSISTED between generations.
//! Creating a new context allocates VRAM and can take 2-5 seconds.
//! Reusing it with a KV cache clear is nearly instant.
//! This is what makes Ollama/LMStudio fast.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use thiserror::Error;

use crate::inference::model::{validate_gguf, ModelError};
use crate::inference::streaming::StreamToken;
use crate::types::message::{Message as ChatMessage, Role as ChatRole};

/// Errors that can occur during inference operations
#[derive(Debug, Error, Clone)]
pub enum EngineError {
    #[error("Backend not initialized")]
    BackendNotInitialized,

    #[error("No model loaded")]
    NoModelLoaded,

    #[error("Failed to initialize backend: {0}")]
    BackendInit(String),

    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    #[error("Failed to create context: {0}")]
    ContextCreate(String),

    #[error("Model validation failed: {0}")]
    ModelValidation(String),

    #[error("Tokenization failed: {0}")]
    Tokenization(String),

    #[error("Inference failed: {0}")]
    Inference(String),

    #[error("Worker thread error: {0}")]
    WorkerError(String),
}

impl From<ModelError> for EngineError {
    fn from(e: ModelError) -> Self {
        EngineError::ModelValidation(e.to_string())
    }
}

/// Generation parameters for inference
#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_k: u32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub seed: u32,
    pub max_context_size: u32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            max_tokens: 4096,       // 4K output with 16K context
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.1,
            seed: 0,
            max_context_size: 16384, // 16K context - validated with LM Studio on 8GB VRAM
        }
    }
}

impl GenerationParams {
    pub fn fast() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.0,
            top_k: 1,
            top_p: 1.0,
            repeat_penalty: 1.0,
            seed: 0,
            max_context_size: 4096,
        }
    }
    
    pub fn balanced() -> Self {
        Self {
            max_tokens: 4096,
            temperature: 0.7,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
            seed: 0,
            max_context_size: 8192,
        }
    }
    
    pub fn quality() -> Self {
        Self {
            max_tokens: 8192,
            temperature: 0.8,
            top_k: 50,
            top_p: 0.95,
            repeat_penalty: 1.1,
            seed: 0,
            max_context_size: 16384,
        }
    }
}

/// Model information after loading
#[derive(Debug, Clone)]
pub struct LoadedModelInfo {
    pub path: String,
    pub vocab_size: i32,
    pub embedding_dim: i32,
    pub context_length: u32,
    pub param_count: u64,
    pub size_bytes: u64,
}

/// Commands sent to the worker thread
enum WorkerCommand {
    Init,
    LoadModel {
        path: PathBuf,
        gpu_layers: u32,
        response_tx: Sender<Result<LoadedModelInfo, EngineError>>,
    },
    UnloadModel,
    Generate {
        messages: Vec<ChatMessage>,
        params: GenerationParams,
        token_tx: Sender<StreamToken>,
        stop_signal: Arc<AtomicBool>,
    },
    Shutdown,
}

/// The main LLM inference engine using llama-cpp-2
pub struct LlamaEngine {
    command_tx: Option<Sender<WorkerCommand>>,
    worker_handle: Option<JoinHandle<()>>,
    model_info: Option<LoadedModelInfo>,
    initialized: bool,
    model_loaded: bool,
}

impl LlamaEngine {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            worker_handle: None,
            model_info: None,
            initialized: false,
            model_loaded: false,
        }
    }

    pub fn init(&mut self) -> Result<(), EngineError> {
        if self.initialized {
            return Ok(());
        }

        let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();

        let handle = thread::spawn(move || {
            worker_thread_main(command_rx);
        });

        self.command_tx = Some(command_tx.clone());
        self.worker_handle = Some(handle);

        command_tx
            .send(WorkerCommand::Init)
            .map_err(|e| EngineError::WorkerError(e.to_string()))?;

        self.initialized = true;
        tracing::info!("LlamaEngine worker thread started");
        Ok(())
    }

    pub fn load_model<P: AsRef<Path>>(
        &mut self,
        path: P,
        gpu_layers: u32,
    ) -> Result<LoadedModelInfo, EngineError> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or(EngineError::BackendNotInitialized)?;

        let path = path.as_ref();
        let _metadata = validate_gguf(path)?;

        let (response_tx, response_rx) = mpsc::channel();

        command_tx
            .send(WorkerCommand::LoadModel {
                path: path.to_path_buf(),
                gpu_layers,
                response_tx,
            })
            .map_err(|e| EngineError::WorkerError(e.to_string()))?;

        let result = response_rx
            .recv()
            .map_err(|e| EngineError::WorkerError(e.to_string()))??;

        self.model_info = Some(result.clone());
        self.model_loaded = true;

        Ok(result)
    }

    pub fn unload_model(&mut self) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(WorkerCommand::UnloadModel);
        }
        self.model_info = None;
        self.model_loaded = false;
        tracing::info!("Model unload requested");
    }

    pub fn model_info(&self) -> Option<&LoadedModelInfo> {
        self.model_info.as_ref()
    }

    pub fn is_model_loaded(&self) -> bool {
        self.model_loaded
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn generate_stream(
        &self,
        prompt: &str,
        params: GenerationParams,
    ) -> Result<(Receiver<StreamToken>, Arc<AtomicBool>), EngineError> {
        let message = ChatMessage::new(ChatRole::User, prompt);
        self.generate_stream_messages(vec![message], params)
    }

    pub fn generate_stream_messages(
        &self,
        messages: Vec<ChatMessage>,
        params: GenerationParams,
    ) -> Result<(Receiver<StreamToken>, Arc<AtomicBool>), EngineError> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or(EngineError::BackendNotInitialized)?;

        if !self.model_loaded {
            return Err(EngineError::NoModelLoaded);
        }

        let (token_tx, token_rx) = mpsc::channel();
        let stop_signal = Arc::new(AtomicBool::new(false));

        command_tx
            .send(WorkerCommand::Generate {
                messages,
                params,
                token_tx,
                stop_signal: stop_signal.clone(),
            })
            .map_err(|e| EngineError::WorkerError(e.to_string()))?;

        Ok((token_rx, stop_signal))
    }
}

impl Default for LlamaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LlamaEngine {
    fn drop(&mut self) {
        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(WorkerCommand::Shutdown);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

// =============================================================================
// Worker thread - owns all llama-cpp state including PERSISTENT context
// =============================================================================

/// Worker state holding all llama-cpp objects.
/// The context is PERSISTENT - created once and reused across generations.
struct WorkerState {
    backend: Option<LlamaBackend>,
    model: Option<LlamaModel>,
    /// PERSISTENT context - reused across generations (the key optimization)
    ctx: Option<LlamaContext<'static>>,
    /// Current context size
    ctx_n_ctx: u32,
    /// Optimal thread count (cached)
    n_threads: i32,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            backend: None,
            model: None,
            ctx: None,
            ctx_n_ctx: 0,
            n_threads: get_optimal_threads(),
        }
    }
}

fn worker_thread_main(command_rx: Receiver<WorkerCommand>) {
    let mut state = WorkerState::new();
    
    // We use unsafe to create a self-referential struct where ctx borrows model.
    // This is safe because:
    // 1. The model outlives the context (we always drop ctx before model)
    // 2. Both live on the same thread
    // 3. The model is never moved while the context exists

    loop {
        match command_rx.recv() {
            Ok(WorkerCommand::Init) => {
                match LlamaBackend::init() {
                    Ok(b) => {
                        state.backend = Some(b);
                        tracing::info!("LlamaBackend initialized");
                    }
                    Err(e) => {
                        tracing::error!("Failed to init backend: {}", e);
                    }
                }
            }
            Ok(WorkerCommand::LoadModel {
                path,
                gpu_layers,
                response_tx,
            }) => {
                // Drop existing context FIRST (before model)
                state.ctx = None;
                state.ctx_n_ctx = 0;
                state.model = None;
                
                match load_model_internal(&state.backend, &path, gpu_layers) {
                    Ok((info, loaded_model)) => {
                        state.model = Some(loaded_model);
                        let _ = response_tx.send(Ok(info));
                    }
                    Err(e) => {
                        let _ = response_tx.send(Err(e));
                    }
                }
            }
            Ok(WorkerCommand::UnloadModel) => {
                // Drop context FIRST, then model
                state.ctx = None;
                state.ctx_n_ctx = 0;
                state.model = None;
                tracing::info!("Model and context unloaded");
            }
            Ok(WorkerCommand::Generate {
                messages,
                params,
                token_tx,
                stop_signal,
            }) => {
                if state.backend.is_none() || state.model.is_none() {
                    let _ = token_tx.send(StreamToken::Error("No model loaded".to_string()));
                    continue;
                }
                
                if let Err(e) = run_generation_persistent(&mut state, &messages, params, &token_tx, &stop_signal) {
                    let _ = token_tx.send(StreamToken::Error(e));
                }
            }
            Ok(WorkerCommand::Shutdown) => {
                // Clean shutdown: drop context first, then model
                state.ctx = None;
                state.model = None;
                state.backend = None;
                tracing::info!("Worker thread shut down");
                break;
            }
            Err(_) => {
                break;
            }
        }
    }
}

// =============================================================================
// Model loading
// =============================================================================

fn load_model_internal(
    backend: &Option<LlamaBackend>,
    path: &Path,
    gpu_layers: u32,
) -> Result<(LoadedModelInfo, LlamaModel), EngineError> {
    let backend = backend.as_ref().ok_or(EngineError::BackendNotInitialized)?;

    let metadata = std::fs::metadata(path)
        .map_err(|e| EngineError::ModelLoad(format!("Cannot read model file: {}", e)))?;

    if metadata.len() == 0 {
        return Err(EngineError::ModelLoad("Model file is empty".to_string()));
    }

    tracing::info!(
        "Loading model: {:?} ({:.2} GB, {} GPU layers)",
        path,
        metadata.len() as f64 / (1024.0 * 1024.0 * 1024.0),
        gpu_layers
    );

    // Model params with mlock to prevent OS paging out weights
    let model_params = LlamaModelParams::default()
        .with_n_gpu_layers(gpu_layers);

    let model = LlamaModel::load_from_file(backend, path, &model_params)
        .map_err(|e| EngineError::ModelLoad(format!("Load failed: {}", e)))?;

    let info = LoadedModelInfo {
        path: path.to_string_lossy().to_string(),
        vocab_size: model.n_vocab(),
        embedding_dim: model.n_embd(),
        context_length: model.n_ctx_train(),
        param_count: model.n_params() as u64,
        size_bytes: model.size() as u64,
    };

    tracing::info!(
        "Model loaded: {:.1}B params, {}K train ctx, {} vocab",
        info.param_count as f64 / 1e9,
        info.context_length / 1024,
        info.vocab_size
    );

    Ok((info, model))
}

// =============================================================================
// Generation with PERSISTENT context (the main performance optimization)
// =============================================================================

fn run_generation_persistent(
    state: &mut WorkerState,
    messages: &[ChatMessage],
    params: GenerationParams,
    tx: &Sender<StreamToken>,
    stop_signal: &Arc<AtomicBool>,
) -> Result<(), String> {
    let start_time = std::time::Instant::now();
    
    let backend = state.backend.as_ref().ok_or("Backend not initialized")?;
    let model = state.model.as_ref().ok_or("Model not loaded")?;

    // Build prompt
    let prompt = match build_chat_prompt_from_messages(model, messages) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Chat template error: {e}, using fallback");
            build_fallback_prompt(messages)
        }
    };

    // Tokenize
    let tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("Tokenization failed: {}", e))?;
    
    let prompt_len = tokens.len() as u32;
    let model_max = model.n_ctx_train();
    
    // Use the SMALLER of model max and user's configured max context
    // This is critical: model may support 128K but user's GPU can only handle 4K
    let effective_max = std::cmp::min(params.max_context_size, model_max);
    
    // Calculate needed context size
    let min_gen = 256u32;
    let needed = std::cmp::min(prompt_len + params.max_tokens, effective_max);
    let needed = std::cmp::max(needed, prompt_len + min_gen);
    let needed = std::cmp::min(needed, effective_max);
    
    // Round up to next standard size for better context reuse
    let n_ctx = pick_context_size(needed, effective_max);
    
    tracing::info!(
        "Prompt: {} tokens, need ctx: {}, model max: {}",
        prompt_len, n_ctx, model_max
    );

    // === THE KEY OPTIMIZATION ===
    // Reuse existing context if it's big enough, otherwise create a new one.
    // Creating a context is SLOW (allocates KV cache in VRAM, 2-5 seconds).
    // Reusing one is INSTANT.
    
    let need_new_ctx = match &state.ctx {
        Some(_) if state.ctx_n_ctx >= n_ctx => {
            tracing::info!(
                "REUSING context ({} >= {}): ~0ms vs 2-5s for new context",
                state.ctx_n_ctx, n_ctx
            );
            false
        }
        Some(_) => {
            tracing::info!(
                "Context too small ({} < {}), recreating...",
                state.ctx_n_ctx, n_ctx
            );
            true
        }
        None => {
            tracing::info!("No existing context, creating new one...");
            true
        }
    };
    
    if need_new_ctx {
        // Drop old context first to free VRAM
        state.ctx = None;
        state.ctx_n_ctx = 0;
        
        let n_threads = state.n_threads;
        let n_batch = calculate_optimal_batch(n_ctx, prompt_len);
        
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(n_ctx).unwrap()))
            .with_n_batch(n_batch)
            .with_n_threads(n_threads)
            .with_n_threads_batch(n_threads);
        
        // SAFETY: The model outlives the context because we always drop ctx before model.
        // Both are owned by WorkerState and we always drop in the right order.
        let model_static: &'static LlamaModel = unsafe { &*(model as *const LlamaModel) };
        
        let ctx = model_static.new_context(backend, ctx_params)
            .map_err(|e| format!("Failed to create context ({}K): {}", n_ctx / 1024, e))?;
        
        state.ctx = Some(ctx);
        state.ctx_n_ctx = n_ctx;
        
        tracing::info!(
            "Context created in {:?}: {}K ctx, {} batch, {} threads",
            start_time.elapsed(), n_ctx / 1024, n_batch, n_threads
        );
    }
    
    let ctx = state.ctx.as_mut().ok_or("Context disappeared")?;
    let actual_n_ctx = state.ctx_n_ctx;
    
    // Clear the KV cache for fresh generation
    ctx.clear_kv_cache();
    
    // Clamp max_tokens to fit in context
    let available = actual_n_ctx.saturating_sub(prompt_len).max(64);
    let effective_max = std::cmp::min(params.max_tokens, available);
    
    if effective_max < params.max_tokens {
        tracing::warn!(
            "Clamped max_tokens: {} -> {} (ctx={}, prompt={})",
            params.max_tokens, effective_max, actual_n_ctx, prompt_len
        );
    }
    
    let mut clamped = params.clone();
    clamped.max_tokens = effective_max;
    
    let ctx_ready_time = start_time.elapsed();
    tracing::info!(
        "Context ready in {:?}: {}K ctx, {} prompt tokens, {} max gen",
        ctx_ready_time, actual_n_ctx / 1024, prompt_len, effective_max
    );

    let n_batch = calculate_optimal_batch(actual_n_ctx, prompt_len);
    run_inference(ctx, model, tokens, clamped, actual_n_ctx, n_batch, tx, stop_signal)
}

/// Pick a good context size (round up for reusability)
fn pick_context_size(needed: u32, max: u32) -> u32 {
    // Round up to standard sizes for better context reuse
    let sizes = [2048, 4096, 8192, 16384, 32768, 65536, 131072];
    for &s in &sizes {
        if s >= needed && s <= max {
            return s;
        }
    }
    std::cmp::min(needed, max)
}

/// Get optimal number of threads
fn get_optimal_threads() -> i32 {
    let logical = std::thread::available_parallelism()
        .map(|p| p.get() as i32)
        .unwrap_or(4);
    
    // Use physical cores (logical / 2 on HT systems)
    // But at least 2, and cap at 16
    let physical = std::cmp::max(2, logical / 2);
    let result = std::cmp::min(physical, 16);
    tracing::info!("Thread config: {} logical -> {} threads", logical, result);
    result
}

/// Calculate optimal batch size
fn calculate_optimal_batch(n_ctx: u32, prompt_len: u32) -> u32 {
    let base = if prompt_len < 512 {
        2048
    } else if prompt_len < 2048 {
        1024
    } else if prompt_len < 4096 {
        512
    } else {
        256
    };
    std::cmp::min(base, n_ctx)
}

// =============================================================================
// Prompt building
// =============================================================================

fn build_chat_prompt_from_messages(
    model: &LlamaModel,
    messages: &[ChatMessage],
) -> Result<String, String> {
    if messages.is_empty() {
        return Err("No messages".to_string());
    }

    let template = model
        .chat_template(None)
        .map_err(|e| format!("Chat template error: {e}"))?;

    let mut chat_messages: Vec<LlamaChatMessage> = Vec::with_capacity(messages.len());
    for msg in messages {
        let role = match msg.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        };
        let chat_msg = LlamaChatMessage::new(role.to_string(), msg.content.clone())
            .map_err(|e| format!("Chat message error: {e}"))?;
        chat_messages.push(chat_msg);
    }

    model
        .apply_chat_template(&template, &chat_messages, true)
        .map_err(|e| format!("Template apply error: {e}"))
}

fn build_fallback_prompt(messages: &[ChatMessage]) -> String {
    let mut out = String::with_capacity(4096);
    for msg in messages {
        let role = match msg.role {
            ChatRole::System => "System",
            ChatRole::User => "User",
            ChatRole::Assistant => "Assistant",
        };
        out.push_str(role);
        out.push_str(": ");
        out.push_str(&msg.content);
        out.push('\n');
    }
    out.push_str("Assistant: ");
    out
}

// =============================================================================
// Inference loop
// =============================================================================

fn run_inference(
    ctx: &mut LlamaContext,
    model: &LlamaModel,
    mut prompt_tokens: Vec<llama_cpp_2::token::LlamaToken>,
    params: GenerationParams,
    n_ctx: u32,
    n_batch: u32,
    tx: &Sender<StreamToken>,
    stop_signal: &Arc<AtomicBool>,
) -> Result<(), String> {
    let inference_start = std::time::Instant::now();
    
    if prompt_tokens.is_empty() {
        return Err("Empty prompt".to_string());
    }

    // Truncate prompt if needed (keep most recent tokens)
    let max_prompt = (n_ctx as usize).saturating_sub(params.max_tokens as usize).max(1);
    if prompt_tokens.len() > max_prompt {
        let start = prompt_tokens.len() - max_prompt;
        prompt_tokens = prompt_tokens[start..].to_vec();
        tracing::warn!("Prompt truncated to {} tokens", prompt_tokens.len());
    }

    // Process prompt in batches
    let batch_size = std::cmp::max(1, n_batch) as usize;
    let mut batch = LlamaBatch::new(batch_size, 1);
    let prompt_len = prompt_tokens.len();

    let prompt_start = std::time::Instant::now();
    for (chunk_index, chunk) in prompt_tokens.chunks(batch_size).enumerate() {
        if stop_signal.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        batch.clear();
        let offset = chunk_index * batch_size;
        for (i, token) in chunk.iter().enumerate() {
            let global_index = offset + i;
            let is_last = global_index + 1 == prompt_len;
            batch
                .add(*token, global_index as i32, &[0], is_last)
                .map_err(|e| format!("Batch add error: {}", e))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| format!("Decode error: {}", e))?;
    }
    
    let prompt_time = prompt_start.elapsed();
    tracing::info!(
        "Prompt: {} tokens in {:?} ({:.0} t/s)",
        prompt_len, prompt_time, prompt_len as f64 / prompt_time.as_secs_f64()
    );

    // Sampler
    let seed = if params.seed == 0 { rand_seed() } else { params.seed };

    let mut sampler = if params.temperature < 0.01 {
        LlamaSampler::greedy()
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::top_k(params.top_k as i32),
            LlamaSampler::top_p(params.top_p, 1),
            LlamaSampler::temp(params.temperature),
            LlamaSampler::dist(seed),
        ])
    };

    let mut n_decoded = prompt_tokens.len() as i32;
    let mut tokens_generated = 0u32;
    let mut utf8_buffer: Vec<u8> = Vec::with_capacity(32);

    let gen_start = std::time::Instant::now();
    
    for _ in 0..params.max_tokens {
        if stop_signal.load(Ordering::Relaxed) {
            break;
        }

        let new_token = sampler.sample(ctx, batch.n_tokens() - 1);
        sampler.accept(new_token);

        if model.is_eog_token(new_token) {
            flush_utf8_buffer(&mut utf8_buffer, tx);
            break;
        }

        tokens_generated += 1;

        let token_bytes = model
            .token_to_bytes(new_token, Special::Tokenize)
            .map_err(|e| format!("Token convert error: {}", e))?;

        utf8_buffer.extend_from_slice(&token_bytes);
        
        if !emit_valid_utf8(&mut utf8_buffer, tx) {
            break;
        }

        batch.clear();
        batch
            .add(new_token, n_decoded, &[0], true)
            .map_err(|e| format!("Batch add error: {}", e))?;

        ctx.decode(&mut batch)
            .map_err(|e| format!("Decode error: {}", e))?;

        n_decoded += 1;
    }

    flush_utf8_buffer(&mut utf8_buffer, tx);

    let gen_time = gen_start.elapsed();
    let total_time = inference_start.elapsed();
    if tokens_generated > 0 {
        tracing::info!(
            "Gen: {} tokens in {:?} ({:.1} t/s), total: {:?}",
            tokens_generated, gen_time,
            tokens_generated as f64 / gen_time.as_secs_f64(),
            total_time
        );
    }

    let _ = tx.send(StreamToken::Done);
    Ok(())
}

// =============================================================================
// UTF-8 helpers
// =============================================================================

#[inline]
fn flush_utf8_buffer(buffer: &mut Vec<u8>, tx: &Sender<StreamToken>) {
    if !buffer.is_empty() {
        if let Ok(s) = String::from_utf8(std::mem::take(buffer)) {
            if !s.is_empty() {
                let _ = tx.send(StreamToken::Token(s));
            }
        }
    }
}

#[inline]
fn emit_valid_utf8(buffer: &mut Vec<u8>, tx: &Sender<StreamToken>) -> bool {
    if let Ok(s) = std::str::from_utf8(buffer) {
        if !s.is_empty() {
            if tx.send(StreamToken::Token(s.to_string())).is_err() {
                return false;
            }
        }
        buffer.clear();
        return true;
    }
    
    // Find valid UTF-8 prefix
    let mut valid_len = buffer.len();
    while valid_len > 0 {
        if std::str::from_utf8(&buffer[..valid_len]).is_ok() {
            break;
        }
        valid_len -= 1;
    }
    
    if valid_len > 0 {
        let s = unsafe { std::str::from_utf8_unchecked(&buffer[..valid_len]) };
        if !s.is_empty() {
            if tx.send(StreamToken::Token(s.to_string())).is_err() {
                return false;
            }
        }
        buffer.drain(..valid_len);
    }
    
    true
}

fn rand_seed() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_new() {
        let engine = LlamaEngine::new();
        assert!(!engine.is_initialized());
        assert!(!engine.is_model_loaded());
        assert!(engine.model_info().is_none());
    }

    #[test]
    fn test_generation_params_default() {
        let params = GenerationParams::default();
        assert_eq!(params.max_tokens, 8192);
        assert_eq!(params.max_context_size, 16384);
        assert!((params.temperature - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_pick_context_size() {
        assert_eq!(pick_context_size(1000, 32768), 2048);
        assert_eq!(pick_context_size(3000, 32768), 4096);
        assert_eq!(pick_context_size(5000, 32768), 8192);
        assert_eq!(pick_context_size(10000, 32768), 16384);
    }

    #[test]
    fn test_unload_without_model() {
        let mut engine = LlamaEngine::new();
        engine.unload_model();
        assert!(!engine.is_model_loaded());
    }
}
