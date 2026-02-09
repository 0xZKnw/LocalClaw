//! HuggingFace model downloader
//!
//! Provides functionality to download GGUF models from HuggingFace Hub.

use crate::storage::get_data_dir;
use std::fs;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Parse a HuggingFace URL to extract model info
#[derive(Debug, Clone)]
pub struct HuggingFaceUrl {
    pub repo_id: String,
    pub filename: String,
    pub revision: String,
}

fn sanitize_local_filename(filename: &str) -> Result<String, String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return Err("Invalid model filename".to_string());
    }

    let no_query = trimmed.split('?').next().unwrap_or(trimmed);
    let no_fragment = no_query.split('#').next().unwrap_or(no_query);
    let no_leading = no_fragment.trim_start_matches('/');

    let flattened = no_leading.replace('\\', "/").replace('/', "__");

    let mut sanitized = String::with_capacity(flattened.len());
    for ch in flattened.chars() {
        let invalid = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid || ch.is_control() {
            sanitized.push('_');
        } else {
            sanitized.push(ch);
        }
    }

    while sanitized.ends_with('.') || sanitized.ends_with(' ') {
        sanitized.pop();
    }

    if sanitized.is_empty() {
        return Err("Invalid model filename".to_string());
    }

    Ok(sanitized)
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
        let url = url.split('?').next().unwrap_or(url);
        let url = url.split('#').next().unwrap_or(url);

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

    let safe_filename = sanitize_local_filename(&filename)?;

    // Get models directory
    let models_dir = get_data_dir()
        .map_err(|e| format!("Failed to get data dir: {}", e))?
        .join("models");

    fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create models dir: {}", e))?;

    let output_path = models_dir.join(&safe_filename);
    let temp_path = models_dir.join(format!("{}.tmp", safe_filename));

    // Check if file already exists and has content
    if output_path.exists() {
        let metadata = fs::metadata(&output_path)
            .map_err(|e| format!("Failed to check existing file: {}", e))?;
        if metadata.len() > 0 {
            tracing::info!("Model already exists: {:?}", output_path);
            return Ok(output_path);
        }
    }

    // Download the file
    tracing::info!("Downloading from: {}", download_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout for large models
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get(&download_url)
        .header("User-Agent", "LocalClaw/0.2.0")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let total_size = response
        .content_length()
        .ok_or("Could not determine file size")?;
    
    tracing::info!("File size: {} bytes ({} MB)", total_size, total_size / 1024 / 1024);

    // Write to temp file first
    let mut temp_file = File::create(&temp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    
    let mut response = response;
    let mut downloaded: u64 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Download error: {}", e))?
    {
        temp_file
            .write_all(&chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }
    temp_file
        .flush()
        .await
        .map_err(|e| format!("Write error: {}", e))?;

    if downloaded != total_size {
        return Err(format!(
            "Download incomplete: got {} bytes, expected {}",
            downloaded, total_size
        ));
    }
    
    // Rename temp file to final location (atomic operation)
    fs::rename(&temp_path, &output_path)
        .map_err(|e| format!("Failed to move downloaded file: {}", e))?;
    
    tracing::info!("Download complete: {:?}", output_path);

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
