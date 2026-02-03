//! Configuration types
//!
//! Application and inference configuration structures.

use serde::{Deserialize, Serialize};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Path to the models directory
    pub models_dir: Option<String>,
    /// Default context size for inference
    pub default_context_size: u32,
    /// Number of GPU layers to offload (0 = CPU only)
    pub gpu_layers: u32,
    /// Enable dark mode
    pub dark_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            models_dir: None,
            default_context_size: 4096,
            gpu_layers: 0,
            dark_mode: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.default_context_size, 4096);
        assert_eq!(config.gpu_layers, 0);
        assert!(config.dark_mode);
        assert!(config.models_dir.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(
            config.default_context_size,
            deserialized.default_context_size
        );
    }
}
