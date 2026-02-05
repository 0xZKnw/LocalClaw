//! Persistent storage
//!
//! This module handles all data persistence for conversations, settings, and model metadata.

use std::path::PathBuf;
use thiserror::Error;

pub mod conversations;
pub mod huggingface;
pub mod models;
pub mod settings;

/// Storage-related errors
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Failed to access data directory: {0}")]
    DataDirError(String),
    #[error("Failed to read file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to serialize/deserialize JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Conversation not found: {0}")]
    ConversationNotFound(String),
}

/// Get the application data directory
///
/// Returns the platform-specific application data directory:
/// - Windows: `C:\Users\{user}\AppData\Roaming\LocaLM\LocaLM`
/// - macOS: `/Users/{user}/Library/Application Support/com.LocaLM.LocaLM`
/// - Linux: `/home/{user}/.local/share/LocaLM`
pub fn get_data_dir() -> Result<PathBuf, StorageError> {
    directories::ProjectDirs::from("com", "LocaLM", "LocaLM")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or_else(|| StorageError::DataDirError("Could not determine data directory".to_string()))
}

/// Initialize the storage directory structure
///
/// Creates the following directories:
/// - `{data_dir}/conversations/` - For conversation JSON files
/// - `{data_dir}/models/` - Default models directory
/// - `{data_dir}/settings.json` - Created by settings module
pub fn init_storage() -> Result<(), StorageError> {
    let data_dir = get_data_dir()?;

    // Create conversations directory
    let conversations_dir = data_dir.join("conversations");
    std::fs::create_dir_all(&conversations_dir)?;

    // Create default models directory
    let models_dir = data_dir.join("models");
    std::fs::create_dir_all(&models_dir)?;

    tracing::info!("Initialized storage at: {}", data_dir.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_retrieval() {
        let result = get_data_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("LocaLM"));
    }

    #[test]
    fn test_init_storage() {
        // We can't easily test init_storage because it uses actual directories
        // but we can verify get_data_dir works
        let data_dir = get_data_dir();
        assert!(data_dir.is_ok());
    }
}
