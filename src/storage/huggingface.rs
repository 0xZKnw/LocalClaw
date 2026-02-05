//! HuggingFace model downloader
//!
//! Provides functionality to download GGUF models from HuggingFace Hub.

use crate::storage::get_data_dir;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Parse a HuggingFace URL to extract model info
#[derive(Debug, Clone)]
pub struct HuggingFaceUrl {
    pub repo_id: String,
    pub filename: String,
    pub revision: String,
}

impl HuggingFaceUrl {
    /// Parse various HuggingFace URL formats
    pub fn parse(url: &str) -> Result<Self, String> {
        // Handle different URL formats:
        // 1. https://huggingface.co/username/repo/blob/main/model.gguf
        // 2. https://huggingface.co/username/repo/resolve/main/model.gguf
        // 3. username/repo/model.gguf
        // 4. username/repo

        let url = url.trim();

        // Try to extract from full URL
        if url.contains("huggingface.co") {
            // Remove base URL
            let path = url
                .replace("https://huggingface.co/", "")
                .replace("http://huggingface.co/", "");

            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() < 2 {
                return Err("Invalid HuggingFace URL format".to_string());
            }

            let username = parts[0];
            let repo = parts[1];
            let repo_id = format!("{}/{}", username, repo);

            // Check if specific file mentioned
            if let Some(filename_pos) = parts.iter().position(|&p| p == "blob" || p == "resolve") {
                if parts.len() > filename_pos + 2 {
                    let revision = parts[filename_pos + 1];
                    let filename = parts[filename_pos + 2..].join("/");
                    return Ok(Self {
                        repo_id,
                        filename,
                        revision: revision.to_string(),
                    });
                }
            }

            // If no specific file, try to find .gguf files
            return Ok(Self {
                repo_id,
                filename: String::new(),
                revision: "main".to_string(),
            });
        }

        // Handle short format: username/repo/filename.gguf or username/repo
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 2 {
            let repo_id = format!("{}/{}", parts[0], parts[1]);
            let filename = if parts.len() > 2 {
                parts[2..].join("/")
            } else {
                String::new()
            };
            return Ok(Self {
                repo_id,
                filename,
                revision: "main".to_string(),
            });
        }

        Err("Could not parse HuggingFace URL".to_string())
    }

    /// Build the download URL for the file
    pub fn download_url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo_id, self.revision, self.filename
        )
    }
}

/// Download a model from HuggingFace
pub async fn download_model(
    url: &str,
    progress_callback: impl Fn(u64, u64) + Send + 'static,
) -> Result<PathBuf, String> {
    let hf_url = HuggingFaceUrl::parse(url)?;

    // If no specific filename, we need to list available GGUF files
    let filename = if hf_url.filename.is_empty() {
        // Try to find available GGUF files in the repo
        match list_gguf_files(&hf_url.repo_id).await {
            Ok(files) => {
                if files.is_empty() {
                    return Err("No GGUF files found in this repository".to_string());
                } else if files.len() == 1 {
                    files[0].clone()
                } else {
                    return Err(format!(
                        "Multiple GGUF files found. Please specify one of: {}",
                        files.join(", ")
                    ));
                }
            }
            Err(e) => return Err(format!("Failed to list files: {}", e)),
        }
    } else {
        hf_url.filename.clone()
    };

    let download_url = format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        hf_url.repo_id, hf_url.revision, filename
    );

    // Get models directory
    let models_dir = get_data_dir()
        .map_err(|e| format!("Failed to get data dir: {}", e))?
        .join("models");

    fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create models dir: {}", e))?;

    let output_path = models_dir.join(&filename);

    // Check if file already exists
    if output_path.exists() {
        return Ok(output_path);
    }

    // Download the file
    let client = reqwest::Client::new();
    let response = client
        .get(&download_url)
        .header("User-Agent", "LocaLM/0.2.0")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let total_size = response
        .content_length()
        .ok_or("Could not determine file size")?;

    let mut file = fs::File::create(&output_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    let bytes = response.bytes().await.map_err(|e| format!("Download error: {}", e))?;
    
    file.write_all(&bytes).map_err(|e| format!("Write error: {}", e))?;
    let downloaded = bytes.len() as u64;
    progress_callback(downloaded, total_size);

    Ok(output_path)
}

/// List available GGUF files in a HuggingFace repository
async fn list_gguf_files(repo_id: &str) -> Result<Vec<String>, String> {
    let api_url = format!("https://huggingface.co/api/models/{}/tree/main", repo_id);

    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch repo info: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()));
    }

    let files: Vec<FileInfo> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let gguf_files: Vec<String> = files
        .into_iter()
        .filter(|f| f.path.ends_with(".gguf"))
        .map(|f| f.path)
        .collect();

    Ok(gguf_files)
}

#[derive(Debug, serde::Deserialize)]
struct FileInfo {
    path: String,
}

/// Get a human-readable size string
pub fn format_size(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes < 1024.0 {
        format!("{} B", bytes as u64)
    } else if bytes < 1024.0 * 1024.0 {
        format!("{:.2} KB", bytes / 1024.0)
    } else if bytes < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB", bytes / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hf_url_full() {
        let url = "https://huggingface.co/TheBloke/Llama-2-7B-GGUF/blob/main/llama-2-7b.Q4_K_M.gguf";
        let parsed = HuggingFaceUrl::parse(url).unwrap();
        assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(parsed.filename, "llama-2-7b.Q4_K_M.gguf");
        assert_eq!(parsed.revision, "main");
    }

    #[test]
    fn test_parse_hf_url_short() {
        let url = "TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf";
        let parsed = HuggingFaceUrl::parse(url).unwrap();
        assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(parsed.filename, "llama-2-7b.Q4_K_M.gguf");
    }

    #[test]
    fn test_parse_hf_url_repo_only() {
        let url = "TheBloke/Llama-2-7B-GGUF";
        let parsed = HuggingFaceUrl::parse(url).unwrap();
        assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(parsed.filename, "");
    }
}
