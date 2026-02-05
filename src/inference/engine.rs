//! Inference engine implementation
//!
//! Core logic for managing llama-cpp context and running inference.
//!
//! # Architecture
//!
//! Since llama-cpp-2 types (`LlamaBackend`, `LlamaModel`, `LlamaContext`) contain
//! raw pointers that are not `Send`, all inference operations run on a dedicated
//! worker thread. The main thread communicates via channels.

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
    /// Maximum number of tokens to generate
    pub max_tokens: u32,
    /// Temperature for sampling (0.0 = greedy, higher = more random)
    pub temperature: f32,
    /// Top-k sampling parameter (0 = disabled)
    pub top_k: u32,
    /// Top-p (nucleus) sampling parameter
    pub top_p: f32,
    /// Repetition penalty
    pub repeat_penalty: f32,
    /// Random seed for sampling (0 = random)
    pub seed: u32,
    /// Context window size
    pub max_context_size: u32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            max_tokens: 65536,
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.1,
            seed: 0,
            max_context_size: 131072,
        }
    }
}

/// Model information after loading
#[derive(Debug, Clone)]
pub struct LoadedModelInfo {
    /// Path to the loaded model
    pub path: String,
    /// Vocabulary size
    pub vocab_size: i32,
    /// Embedding dimension
    pub embedding_dim: i32,
    /// Training context length
    pub context_length: u32,
    /// Total parameter count
    pub param_count: u64,
    /// Model size in bytes
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
        prompt: String,
        params: GenerationParams,
        token_tx: Sender<StreamToken>,
        stop_signal: Arc<AtomicBool>,
    },
    Shutdown,
}

/// The main LLM inference engine using llama-cpp-2
///
/// Uses a dedicated worker thread for all llama-cpp operations since
/// the underlying types are not Send.
pub struct LlamaEngine {
    /// Channel to send commands to the worker thread
    command_tx: Option<Sender<WorkerCommand>>,
    /// Handle to the worker thread
    worker_handle: Option<JoinHandle<()>>,
    /// Cached model info (updated after load)
    model_info: Option<LoadedModelInfo>,
    /// Whether backend is initialized
    initialized: bool,
    /// Whether a model is loaded
    model_loaded: bool,
}

impl LlamaEngine {
    /// Creates a new uninitialized engine
    pub fn new() -> Self {
        Self {
            command_tx: None,
            worker_handle: None,
            model_info: None,
            initialized: false,
            model_loaded: false,
        }
    }

    /// Initializes the llama.cpp backend
    ///
    /// Must be called before loading models or running inference.
    /// Spawns a dedicated worker thread for all llama-cpp operations.
    pub fn init(&mut self) -> Result<(), EngineError> {
        if self.initialized {
            return Ok(());
        }

        let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();

        // Spawn worker thread that owns the backend and model
        let handle = thread::spawn(move || {
            worker_thread_main(command_rx);
        });

        self.command_tx = Some(command_tx.clone());
        self.worker_handle = Some(handle);

        // Send init command to worker
        command_tx
            .send(WorkerCommand::Init)
            .map_err(|e| EngineError::WorkerError(e.to_string()))?;

        self.initialized = true;
        tracing::info!("LlamaEngine worker thread started");
        Ok(())
    }

    /// Loads a GGUF model from the specified path
    ///
    /// # Arguments
    /// * `path` - Path to the GGUF model file
    /// * `gpu_layers` - Number of layers to offload to GPU (0 = CPU only, high value = all to GPU)
    ///
    /// # Returns
    /// * `Ok(LoadedModelInfo)` - Information about the loaded model
    /// * `Err(EngineError)` - If model loading fails
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

        // Validate GGUF file first (on main thread, just file I/O)
        let _metadata = validate_gguf(path)?;
        tracing::debug!("GGUF validation passed for {:?}", path);

        // Create response channel
        let (response_tx, response_rx) = mpsc::channel();

        // Send load command to worker
        command_tx
            .send(WorkerCommand::LoadModel {
                path: path.to_path_buf(),
                gpu_layers,
                response_tx,
            })
            .map_err(|e| EngineError::WorkerError(e.to_string()))?;

        // Wait for response
        let result = response_rx
            .recv()
            .map_err(|e| EngineError::WorkerError(e.to_string()))??;

        self.model_info = Some(result.clone());
        self.model_loaded = true;

        Ok(result)
    }

    /// Unloads the current model and frees VRAM
    pub fn unload_model(&mut self) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(WorkerCommand::UnloadModel);
        }
        self.model_info = None;
        self.model_loaded = false;
        tracing::info!("Model unload requested");
    }

    /// Returns information about the currently loaded model
    pub fn model_info(&self) -> Option<&LoadedModelInfo> {
        self.model_info.as_ref()
    }

    /// Returns true if a model is currently loaded
    pub fn is_model_loaded(&self) -> bool {
        self.model_loaded
    }

    /// Returns true if the backend is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Generates text with streaming output
    ///
    /// # Arguments
    /// * `prompt` - The input prompt text
    /// * `params` - Generation parameters
    ///
    /// # Returns
    /// * `Ok((Receiver<StreamToken>, Arc<AtomicBool>))` - Receiver for streaming tokens and stop signal
    /// * `Err(EngineError)` - If generation setup fails
    pub fn generate_stream(
        &self,
        prompt: &str,
        params: GenerationParams,
    ) -> Result<(Receiver<StreamToken>, Arc<AtomicBool>), EngineError> {
        let command_tx = self
            .command_tx
            .as_ref()
            .ok_or(EngineError::BackendNotInitialized)?;

        if !self.model_loaded {
            return Err(EngineError::NoModelLoaded);
        }

        // Create channel for streaming tokens
        let (token_tx, token_rx) = mpsc::channel();

        // Create stop signal
        let stop_signal = Arc::new(AtomicBool::new(false));

        // Send generate command to worker
        command_tx
            .send(WorkerCommand::Generate {
                prompt: prompt.to_string(),
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
        // Send shutdown command
        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(WorkerCommand::Shutdown);
        }
        // Wait for worker thread to finish
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Worker thread main loop
///
/// Owns the LlamaBackend and LlamaModel, processes commands from main thread.
fn worker_thread_main(command_rx: Receiver<WorkerCommand>) {
    let mut backend: Option<LlamaBackend> = None;
    let mut model: Option<LlamaModel> = None;

    loop {
        match command_rx.recv() {
            Ok(WorkerCommand::Init) => match LlamaBackend::init() {
                Ok(b) => {
                    backend = Some(b);
                    tracing::info!("LlamaBackend initialized in worker thread");
                }
                Err(e) => {
                    tracing::error!("Failed to init backend: {}", e);
                }
            },
            Ok(WorkerCommand::LoadModel {
                path,
                gpu_layers,
                response_tx,
            }) => {
                let result = load_model_internal(&backend, &path, gpu_layers);
                match &result {
                    Ok(info) => {
                        // Actually load the model and store it
                        if let Some(ref b) = backend {
                            let model_params =
                                LlamaModelParams::default().with_n_gpu_layers(gpu_layers);
                            match LlamaModel::load_from_file(b, &path, &model_params) {
                                Ok(m) => {
                                    model = Some(m);
                                    tracing::info!("Model loaded: {}", info.path);
                                }
                                Err(e) => {
                                    let _ = response_tx
                                        .send(Err(EngineError::ModelLoad(e.to_string())));
                                    continue;
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
                let _ = response_tx.send(result);
            }
            Ok(WorkerCommand::UnloadModel) => {
                model = None;
                tracing::info!("Model unloaded in worker thread");
            }
            Ok(WorkerCommand::Generate {
                prompt,
                params,
                token_tx,
                stop_signal,
            }) => {
                if let (Some(ref b), Some(ref m)) = (&backend, &model) {
                    if let Err(e) = run_generation(b, m, &prompt, params, &token_tx, &stop_signal) {
                        let _ = token_tx.send(StreamToken::Error(e));
                    }
                } else {
                    let _ = token_tx.send(StreamToken::Error("No model loaded".to_string()));
                }
            }
            Ok(WorkerCommand::Shutdown) => {
                tracing::info!("Worker thread shutting down");
                break;
            }
            Err(_) => {
                // Channel closed, exit
                tracing::debug!("Command channel closed, worker exiting");
                break;
            }
        }
    }
}

/// Load model and extract info (helper for worker thread)
fn load_model_internal(
    backend: &Option<LlamaBackend>,
    path: &Path,
    gpu_layers: u32,
) -> Result<LoadedModelInfo, EngineError> {
    let backend = backend.as_ref().ok_or(EngineError::BackendNotInitialized)?;

    let model_params = LlamaModelParams::default().with_n_gpu_layers(gpu_layers);

    let model = LlamaModel::load_from_file(backend, path, &model_params)
        .map_err(|e| EngineError::ModelLoad(e.to_string()))?;

    let info = LoadedModelInfo {
        path: path.to_string_lossy().to_string(),
        vocab_size: model.n_vocab(),
        embedding_dim: model.n_embd(),
        context_length: model.n_ctx_train(),
        param_count: model.n_params() as u64,
        size_bytes: model.size() as u64,
    };

    tracing::info!(
        "Model info extracted: {} ({} params, {} vocab, {} ctx)",
        info.path,
        info.param_count,
        info.vocab_size,
        info.context_length
    );

    // Note: We load the model twice - once for info, once to keep.
    // This is inefficient but keeps the code simple. Could be optimized.
    Ok(info)
}

/// Run text generation (called from worker thread)
fn run_generation(
    backend: &LlamaBackend,
    model: &LlamaModel,
    prompt: &str,
    params: GenerationParams,
    tx: &Sender<StreamToken>,
    stop_signal: &Arc<AtomicBool>,
) -> Result<(), String> {
    let prompt = match build_chat_prompt(model, prompt) {
        Ok(chat_prompt) => chat_prompt,
        Err(error) => {
            tracing::warn!("Chat template not applied: {error}");
            prompt.to_string()
        }
    };

    // Create context for this generation
    // Use context size from settings, or model's max if not specified
    let n_ctx = std::cmp::min(params.max_context_size, model.n_ctx_train());
    let n_ctx = std::cmp::max(n_ctx, 2048); // Minimum 2K context

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(NonZeroU32::new(n_ctx).unwrap()))
        .with_n_batch(512);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| format!("Failed to create context: {}", e))?;

    // Tokenize the prompt
    let tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("Failed to tokenize: {}", e))?;

    tracing::debug!("Tokenized prompt into {} tokens", tokens.len());

    run_inference(&mut ctx, model, tokens, params, tx, stop_signal)
}

fn build_chat_prompt(model: &LlamaModel, prompt: &str) -> Result<String, String> {
    let template = model
        .chat_template(None)
        .map_err(|e| format!("Failed to load chat template: {e}"))?;
    let user_message = LlamaChatMessage::new("user".to_string(), prompt.to_string())
        .map_err(|e| format!("Failed to build chat message: {e}"))?;
    model
        .apply_chat_template(&template, &[user_message], true)
        .map_err(|e| format!("Failed to apply chat template: {e}"))
}
/// Runs the inference loop
fn run_inference(
    ctx: &mut LlamaContext,
    model: &LlamaModel,
    prompt_tokens: Vec<llama_cpp_2::token::LlamaToken>,
    params: GenerationParams,
    tx: &Sender<StreamToken>,
    stop_signal: &Arc<AtomicBool>,
) -> Result<(), String> {
    // Create batch and add prompt tokens
    let mut batch = LlamaBatch::new(512, 1);

    for (i, token) in prompt_tokens.iter().enumerate() {
        let is_last = i == prompt_tokens.len() - 1;
        batch
            .add(*token, i as i32, &[0], is_last)
            .map_err(|e| format!("Failed to add token to batch: {}", e))?;
    }

    // Process prompt
    ctx.decode(&mut batch)
        .map_err(|e| format!("Failed to decode prompt: {}", e))?;

    // Create sampler chain
    let seed = if params.seed == 0 {
        rand_seed()
    } else {
        params.seed
    };

    let mut sampler = if params.temperature < 0.01 {
        // Use greedy sampling for very low temperature
        LlamaSampler::greedy()
    } else {
        // Chain samplers for controlled randomness
        LlamaSampler::chain_simple([
            LlamaSampler::top_k(params.top_k as i32),
            LlamaSampler::top_p(params.top_p, 1),
            LlamaSampler::temp(params.temperature),
            LlamaSampler::dist(seed),
        ])
    };

    let mut n_decoded = prompt_tokens.len() as i32;

    // Buffer for handling incomplete UTF-8 sequences
    let mut utf8_buffer: Vec<u8> = Vec::new();

    // Generation loop
    for _ in 0..params.max_tokens {
        // Check stop signal
        if stop_signal.load(Ordering::Relaxed) {
            tracing::debug!("Generation stopped by user");
            break;
        }

        // Sample next token
        let new_token = sampler.sample(ctx, batch.n_tokens() - 1);
        sampler.accept(new_token);

        // Check for end of generation
        if model.is_eog_token(new_token) {
            tracing::debug!("End of generation token encountered");
            // Flush any remaining UTF-8 buffer on end of generation
            if !utf8_buffer.is_empty() {
                if let Ok(s) = String::from_utf8(utf8_buffer.clone()) {
                    if !s.is_empty() {
                        let _ = tx.send(StreamToken::Token(s));
                    }
                }
                utf8_buffer.clear();
            }
            break;
        }

        // Convert token to bytes instead of string
        let token_bytes = model
            .token_to_bytes(new_token, Special::Tokenize)
            .map_err(|e| format!("Failed to convert token to bytes: {}", e))?;

        // Accumulate bytes in the buffer
        utf8_buffer.extend_from_slice(&token_bytes);

        // Try to extract valid UTF-8 from the buffer
        // Find the longest valid UTF-8 prefix
        if let Ok(s) = String::from_utf8(utf8_buffer.clone()) {
            // All accumulated bytes form valid UTF-8
            if !s.is_empty() {
                if tx.send(StreamToken::Token(s)).is_err() {
                    // Receiver dropped, stop generation
                    tracing::debug!("Receiver dropped, stopping generation");
                    break;
                }
            }
            utf8_buffer.clear();
        } else {
            // Invalid UTF-8. Try to find the longest valid prefix and emit it,
            // keeping only the incomplete suffix in the buffer.
            let mut valid_len = 0;
            for i in (1..=utf8_buffer.len()).rev() {
                if let Ok(s) = String::from_utf8(utf8_buffer[..i].to_vec()) {
                    valid_len = i;
                    if !s.is_empty() {
                        if tx.send(StreamToken::Token(s)).is_err() {
                            // Receiver dropped, stop generation
                            tracing::debug!("Receiver dropped, stopping generation");
                            break;
                        }
                    }
                    break;
                }
            }

            // Keep only the incomplete suffix
            if valid_len > 0 {
                utf8_buffer = utf8_buffer[valid_len..].to_vec();
            }
            // If valid_len == 0, we keep all bytes (they're an incomplete sequence)
        }

        // Prepare batch for next iteration
        batch.clear();
        batch
            .add(new_token, n_decoded, &[0], true)
            .map_err(|e| format!("Failed to add token to batch: {}", e))?;

        // Decode
        ctx.decode(&mut batch)
            .map_err(|e| format!("Failed to decode: {}", e))?;

        n_decoded += 1;
    }

    // Flush any remaining UTF-8 buffer before completion
    if !utf8_buffer.is_empty() {
        if let Ok(s) = String::from_utf8(utf8_buffer) {
            if !s.is_empty() {
                let _ = tx.send(StreamToken::Token(s));
            }
        }
    }

    // Send done signal
    let _ = tx.send(StreamToken::Done);

    Ok(())
}

/// Generates a random seed using system entropy
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
        assert_eq!(params.max_tokens, 65536);
        assert_eq!(params.max_context_size, 131072);
        assert!((params.temperature - 0.7).abs() < 0.001);
        assert_eq!(params.top_k, 40);
        assert!((params.top_p - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_unload_without_model() {
        let mut engine = LlamaEngine::new();
        // Should not panic
        engine.unload_model();
        assert!(!engine.is_model_loaded());
    }
}
