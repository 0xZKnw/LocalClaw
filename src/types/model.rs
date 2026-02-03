//! Model types
//!
//! Defines model metadata and configuration structures.

use serde::{Deserialize, Serialize};

/// Information about a loaded model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Display name of the model
    pub name: String,
    /// Path to the GGUF file
    pub path: String,
    /// Model size in bytes
    pub size_bytes: u64,
    /// Number of parameters (if known)
    pub parameters: Option<u64>,
}
