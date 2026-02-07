//! GPU detection and management
//!
//! Detects available GPUs and their capabilities for model acceleration.

#[cfg(target_os = "windows")]
use std::process::Command;

/// GPU information
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub name: String,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub vram_usage_available: bool,
    pub is_available: bool,
}

/// Get total dedicated VRAM in GB (returns 0.0 if detection fails)
pub fn get_total_vram_gb() -> Option<f64> {
    let gpu = detect_gpu();
    if gpu.is_available && gpu.vram_total_mb > 0 {
        Some(gpu.vram_total_mb as f64 / 1024.0)
    } else {
        None
    }
}

/// Detect available GPU (best effort)
pub fn detect_gpu() -> GpuInfo {
    #[cfg(target_os = "windows")]
    {
        return detect_gpu_windows();
    }

    #[cfg(not(target_os = "windows"))]
    {
        GpuInfo {
            name: "GPU non détecté".to_string(),
            vram_total_mb: 0,
            vram_used_mb: 0,
            vram_usage_available: false,
            is_available: false,
        }
    }
}

#[cfg(target_os = "windows")]
fn detect_gpu_windows() -> GpuInfo {
    if let Some(info) = detect_gpu_nvidia_smi() {
        return info;
    }

    if let Some(info) = detect_gpu_wmic() {
        return info;
    }

    GpuInfo {
        name: "GPU non détecté".to_string(),
        vram_total_mb: 0,
        vram_used_mb: 0,
        vram_usage_available: false,
        is_available: false,
    }
}

#[cfg(target_os = "windows")]
fn detect_gpu_nvidia_smi() -> Option<GpuInfo> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|l| !l.trim().is_empty())?;
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }

    let name = parts[0].to_string();
    let vram_total_mb = parts[1].parse::<u64>().ok()?;
    let vram_used_mb = parts[2].parse::<u64>().ok()?;

    Some(GpuInfo {
        name,
        vram_total_mb,
        vram_used_mb,
        vram_usage_available: true,
        is_available: true,
    })
}

#[cfg(target_os = "windows")]
fn detect_gpu_wmic() -> Option<GpuInfo> {
    let output = Command::new("wmic")
        .args([
            "path",
            "Win32_VideoController",
            "get",
            "Name,AdapterRAM",
            "/Format:List",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut name: Option<String> = None;
    let mut adapter_ram_bytes: Option<u64> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Name=") {
            let value = line.trim_start_matches("Name=").trim();
            if !value.is_empty() {
                name = Some(value.to_string());
            }
        } else if line.starts_with("AdapterRAM=") {
            let value = line.trim_start_matches("AdapterRAM=").trim();
            if let Ok(bytes) = value.parse::<u64>() {
                adapter_ram_bytes = Some(bytes);
            }
        }

        if name.is_some() && adapter_ram_bytes.is_some() {
            break;
        }
    }

    let name = name?;
    let vram_total_mb = adapter_ram_bytes.unwrap_or(0) / 1024 / 1024;

    Some(GpuInfo {
        name,
        vram_total_mb,
        vram_used_mb: 0,
        vram_usage_available: false,
        is_available: true,
    })
}
