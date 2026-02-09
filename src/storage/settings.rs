//! Settings storage
//!
//! Manages persistence of user preferences and application settings.

use crate::storage::{get_data_dir, StorageError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Temperature parameter for text generation (0.0 - 2.0)
    pub temperature: f32,
    /// Top-p (nucleus sampling) parameter (0.0 - 1.0)
    pub top_p: f32,
    /// Top-k sampling parameter
    pub top_k: u32,
    /// Maximum number of tokens to generate
    pub max_tokens: u32,
    /// Context window size
    pub context_size: u32,
    /// System prompt prepended to conversations
    pub system_prompt: String,
    /// Number of GPU layers to offload (0 = CPU only)
    pub gpu_layers: u32,
    /// Directory where model files (.gguf) are stored
    pub models_directory: PathBuf,
    /// UI theme: "dark" or "light"
    pub theme: String,
    /// Font size: "small", "medium", or "large"
    pub font_size: String,
    /// Exa MCP server URL
    #[serde(default)]
    pub exa_mcp_url: String,
    /// Last loaded model path (for auto-loading on startup)
    #[serde(default)]
    pub last_model_path: Option<String>,
    /// Auto-load last model on startup
    #[serde(default = "default_auto_load")]
    pub auto_load_model: bool,
    /// UI and agent language: "fr" or "en"
    #[serde(default = "default_language")]
    pub language: String,
    /// Auto-approve ALL tool calls without asking (dangerous but convenient)
    #[serde(default)]
    pub auto_approve_all_tools: bool,
    /// List of tool names that are auto-approved (allowlist)
    #[serde(default)]
    pub tool_allowlist: Vec<String>,
    /// List of disabled MCP server IDs
    #[serde(default)]
    pub disabled_mcp_servers: Vec<String>,
    /// OpenRouter model to use for ai_consult tool (default: openrouter/pony-alpha)
    #[serde(default = "default_openrouter_model")]
    pub openrouter_model: String,
}

fn default_auto_load() -> bool {
    true
}

fn default_language() -> String {
    "fr".to_string()
}

fn default_openrouter_model() -> String {
    "openrouter/pony-alpha".to_string()
}

/// Default system prompt from code. Used on every app load so the prompt always matches the code.
pub fn default_system_prompt() -> String {
    default_system_prompt_for_lang("fr")
}

/// Build system prompt for a specific language
pub fn default_system_prompt_for_lang(lang: &str) -> String {
    let os_name = match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" => "Linux",
        other => other,
    };
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "unknown".to_string());
    let sep = if std::env::consts::OS == "windows" {
        "\\"
    } else {
        "/"
    };
    let cmd_info = if std::env::consts::OS == "windows" {
        "\n- This is Windows. Use PowerShell commands: dir, Get-ChildItem, Get-Content, etc.\n- Do NOT use Unix commands (ls, cat, grep). They won't work."
    } else {
        "\n- Use standard Unix commands: ls, cat, grep, find, etc."
    };
    let cmd_example = if std::env::consts::OS == "windows" {
        "dir"
    } else {
        "ls -la"
    };

    let response_lang_instruction = if lang == "en" {
        "Always respond in English."
    } else {
        "Always respond in French (toujours repondre en francais)."
    };

    format!(
        r#"You are LocaLM, a helpful AI assistant running locally on the user's machine.
{response_lang}

## System Environment

- OS: {os_name} ({arch})
- Home directory: {home}
- Desktop: {home}{sep}Desktop
- Documents: {home}{sep}Documents
- Downloads: {home}{sep}Downloads
- Path separator: {sep}

You know the user's home directory: {home}. Use it directly, never ask.

## Tools

You have these tools. Use them IMMEDIATELY when relevant — do NOT ask the user for information you can look up yourself.

### web_search
Search the web for current information.
```json
{{"tool": "web_search", "params": {{"query": "search terms", "num_results": 5}}}}
```

### file_read
Read a file's content. You know the user's paths, use them.
```json
{{"tool": "file_read", "params": {{"path": "{home}{sep}Desktop{sep}example.txt"}}}}
```

### file_list
List a directory's contents.
```json
{{"tool": "file_list", "params": {{"path": "{home}{sep}Desktop"}}}}
```

### command
Execute a shell command.{cmd_info}
```json
{{"tool": "command", "params": {{"command": "{cmd_example}", "timeout_secs": 30}}}}
```

## Rules

1. **ACT, don't ask.** If the user says "list my desktop", use file_list with their Desktop path immediately. Do NOT ask them for the path.
2. **Use tools proactively.** If you need info, use a tool. Don't say "I can't access your files" — you CAN.
3. **Derive paths.** Desktop = Home{sep}Desktop, Documents = Home{sep}Documents, etc.
4. **One tool per message.** Call one tool, wait for the result, then respond or call another.
5. **Be concise.** Give direct, useful answers. No unnecessary preamble.
6. **Handle errors.** If a tool fails, try an alternative approach.
7. **Think internally.** Use <think>...</think> for your reasoning. The user sees it as a collapsible block.
8. **{response_lang}**

## ⚠️ MANDATORY VERIFICATION (ANTI-HALLUCINATION)

**CRITICAL RULES:**
- NEVER say "done" or "file created" BEFORE receiving system confirmation
- AFTER each creation/modification, VERIFY with file_list or file_read that it actually exists
- If you haven't seen "[TOOL_RESULT]" or a system result, the tool was NOT executed
- NEVER generate fake tool results - the SYSTEM executes them, not you
- If you need to confirm an action, USE a verification tool FIRST"#,
        response_lang = response_lang_instruction,
        os_name = os_name,
        arch = std::env::consts::ARCH,
        home = home,
        sep = sep,
        cmd_info = cmd_info,
        cmd_example = cmd_example,
    )
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            max_tokens: 4096,    // 4K output - OK with 16K context
            context_size: 16384, // 16K context - user confirmed 36 tok/s in LM Studio with 16K on 8GB VRAM
            system_prompt: default_system_prompt(),
            gpu_layers: 99, // Offload all layers to GPU by default
            models_directory: get_data_dir()
                .ok()
                .map(|d| d.join("models"))
                .unwrap_or_else(|| PathBuf::from("./models")),
            theme: "dark".to_string(),
            font_size: "medium".to_string(),
            exa_mcp_url: "https://mcp.exa.ai/mcp".to_string(),
            last_model_path: None,
            auto_load_model: true,
            language: "fr".to_string(),
            auto_approve_all_tools: false,
            tool_allowlist: Vec::new(),
            disabled_mcp_servers: Vec::new(),
            openrouter_model: default_openrouter_model(),
        }
    }
}

impl AppSettings {
    /// Validate settings values
    ///
    /// Ensures all parameters are within acceptable ranges.
    /// Also caps context size based on available VRAM to prevent KV cache overflow.
    pub fn validate(&mut self) {
        self.temperature = self.temperature.clamp(0.0, 2.0);
        self.top_p = self.top_p.clamp(0.0, 1.0);

        if self.top_k == 0 {
            self.top_k = 40;
        }

        self.max_tokens = self.max_tokens.clamp(1, 65536);

        // Valid context sizes
        let valid_context_sizes = [2048, 4096, 8192, 16384, 32768, 65536, 131072];
        if !valid_context_sizes.contains(&self.context_size) {
            self.context_size = *valid_context_sizes
                .iter()
                .min_by_key(|&&size| (size as i64 - self.context_size as i64).abs())
                .unwrap_or(&4096);
        }

        // === VRAM-aware context cap ===
        // Prevent KV cache from overflowing dedicated VRAM.
        // 7B Q4_K_M ~4.1 GB; 16K context KV cache ~2 GB → fits in 8 GB.
        let max_safe_context = get_vram_safe_context_size();
        if self.context_size > max_safe_context {
            tracing::warn!(
                "Context size {} too large for available VRAM, capping to {}",
                self.context_size,
                max_safe_context
            );
            self.context_size = max_safe_context;
        }

        // Cap max_tokens to context_size (can't generate more than context allows)
        if self.max_tokens > self.context_size {
            self.max_tokens = self.context_size / 2;
        }

        if self.theme != "dark" && self.theme != "light" {
            self.theme = "dark".to_string();
        }

        if !["small", "medium", "large"].contains(&self.font_size.as_str()) {
            self.font_size = "medium".to_string();
        }

        if self.exa_mcp_url.trim().is_empty() {
            self.exa_mcp_url = "https://mcp.exa.ai/mcp".to_string();
        }

        if self.language != "fr" && self.language != "en" {
            self.language = "fr".to_string();
        }
    }
}

/// Estimate the maximum safe context size based on available VRAM.
/// This prevents the KV cache from spilling into shared GPU memory (RAM), which is slow.
/// Tuned so 8 GB VRAM allows 16K context (7B model ~4.1 GB + 16K KV ~2 GB).
fn get_vram_safe_context_size() -> u32 {
    let vram_gb = crate::system::gpu::get_total_vram_gb().unwrap_or(0.0);

    if vram_gb <= 0.0 {
        return 16384; // default when VRAM unknown
    }

    // Heuristic: 50% VRAM for model, 50% for KV cache. 7B 16K ≈ 2 GB KV.
    // Per 1K context for 7B: ~128 MB. Use 128 so 8 GB -> 4 GB for KV -> 32K cap.
    let vram_for_kv = vram_gb * 0.5;
    let max_ctx_k = (vram_for_kv * 1024.0 / 128.0) as u32;
    let max_ctx = max_ctx_k * 1024;

    let sizes = [131072, 65536, 32768, 16384, 8192, 4096, 2048];
    for &s in &sizes {
        if s <= max_ctx {
            tracing::info!("VRAM: {:.1} GB -> max safe context: {}K", vram_gb, s / 1024);
            return s;
        }
    }

    2048
}

/// Get the settings file path
fn get_settings_path() -> Result<PathBuf, StorageError> {
    Ok(get_data_dir()?.join("settings.json"))
}

/// Load settings from disk
///
/// Returns default settings if the file doesn't exist or is corrupted
pub fn load_settings() -> AppSettings {
    match load_settings_internal() {
        Ok(settings) => settings,
        Err(e) => {
            tracing::warn!("Failed to load settings, using defaults: {}", e);
            AppSettings::default()
        }
    }
}

/// Internal settings loading with error propagation
fn load_settings_internal() -> Result<AppSettings, StorageError> {
    let path = get_settings_path()?;

    if !path.exists() {
        tracing::info!("Settings file not found, using defaults");
        return Ok(AppSettings::default());
    }

    let json = fs::read_to_string(&path)?;
    let mut settings: AppSettings = serde_json::from_str(&json)?;

    // Always use system prompt from code so app reflects current version on reload
    settings.system_prompt = default_system_prompt_for_lang(&settings.language);

    // Validate loaded settings
    settings.validate();

    tracing::debug!("Loaded settings from disk");
    Ok(settings)
}

/// Save settings to disk
pub fn save_settings(settings: &AppSettings) -> Result<(), StorageError> {
    let path = get_settings_path()?;

    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(settings)?;
    fs::write(path, json)?;

    tracing::debug!("Saved settings to disk");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.temperature, 0.7);
        assert_eq!(settings.top_p, 0.9);
        assert_eq!(settings.top_k, 40);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.font_size, "medium");
    }

    #[test]
    fn test_settings_validation() {
        let mut settings = AppSettings::default();

        // Test temperature clamping
        settings.temperature = 5.0;
        settings.validate();
        assert_eq!(settings.temperature, 2.0);

        settings.temperature = -1.0;
        settings.validate();
        assert_eq!(settings.temperature, 0.0);

        // Test top_p clamping
        settings.top_p = 2.0;
        settings.validate();
        assert_eq!(settings.top_p, 1.0);

        // Test invalid theme
        settings.theme = "invalid".to_string();
        settings.validate();
        assert_eq!(settings.theme, "dark");

        // Test invalid font size
        settings.font_size = "huge".to_string();
        settings.validate();
        assert_eq!(settings.font_size, "medium");
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings::default();

        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(settings.temperature, deserialized.temperature);
        assert_eq!(settings.top_p, deserialized.top_p);
        assert_eq!(settings.theme, deserialized.theme);
    }

    #[test]
    fn test_settings_persistence() {
        // Test that settings can be saved and loaded
        let settings = AppSettings::default();

        // Serialize and deserialize
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let mut loaded: AppSettings = serde_json::from_str(&json).unwrap();
        loaded.validate();

        assert_eq!(settings.temperature, loaded.temperature);
        assert_eq!(settings.theme, loaded.theme);
    }
}
