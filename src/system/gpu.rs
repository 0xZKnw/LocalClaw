//! GPU detection and management
//!
//! Detects available GPUs and their capabilities for model acceleration.

#[cfg(any(target_os = "windows", target_os = "macos"))]
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

    #[cfg(target_os = "macos")]
    {
        return detect_gpu_macos();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        GpuInfo {
            name: "GPU not detected".to_string(),
            vram_total_mb: 0,
            vram_used_mb: 0,
            vram_usage_available: false,
            is_available: false,
        }
    }
}

// =============================================================================
// macOS GPU detection
// =============================================================================

#[cfg(target_os = "macos")]
fn detect_gpu_macos() -> GpuInfo {
    // Try system_profiler for GPU info
    if let Some(info) = detect_gpu_system_profiler() {
        return info;
    }

    // Fallback: detect Apple Silicon via sysctl
    if let Some(info) = detect_gpu_apple_silicon() {
        return info;
    }

    GpuInfo {
        name: "GPU not detected".to_string(),
        vram_total_mb: 0,
        vram_used_mb: 0,
        vram_usage_available: false,
        is_available: false,
    }
}

/// Detect GPU using system_profiler SPDisplaysDataType
#[cfg(target_os = "macos")]
fn detect_gpu_system_profiler() -> Option<GpuInfo> {
    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpu_name: Option<String> = None;
    let mut vram_mb: Option<u64> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // GPU name: "Chipset Model: Apple M2 Pro" or "Chipset Model: AMD Radeon Pro 5500M"
        if trimmed.starts_with("Chipset Model:") {
            let name = trimmed.trim_start_matches("Chipset Model:").trim();
            if !name.is_empty() {
                gpu_name = Some(name.to_string());
            }
        }

        // VRAM: "VRAM (Total): 16 GB" or "VRAM (Dynamic, Max): 48 GB"
        if trimmed.contains("VRAM") && trimmed.contains(":") {
            let after_colon = trimmed.split(':').nth(1).unwrap_or("").trim();
            // Parse "16 GB" or "4096 MB"
            let parts: Vec<&str> = after_colon.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(val) = parts[0].parse::<u64>() {
                    vram_mb = Some(match parts[1].to_uppercase().as_str() {
                        "GB" => val * 1024,
                        "MB" => val,
                        _ => val,
                    });
                }
            }
        }
    }

    let name = gpu_name?;

    // For Apple Silicon, VRAM is unified memory — get total system RAM as VRAM
    let is_apple_silicon = name.contains("Apple");
    if is_apple_silicon && vram_mb.is_none() {
        vram_mb = get_macos_total_ram_mb();
    }

    Some(GpuInfo {
        name: format!("{} (Metal)", name),
        vram_total_mb: vram_mb.unwrap_or(0),
        vram_used_mb: 0,
        vram_usage_available: false,
        is_available: true,
    })
}

/// Detect Apple Silicon GPU by checking if the chip is Apple-based via sysctl
#[cfg(target_os = "macos")]
fn detect_gpu_apple_silicon() -> Option<GpuInfo> {
    // Check CPU brand to detect Apple Silicon
    let output = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let brand = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !brand.contains("Apple") {
        return None;
    }

    let total_ram_mb = get_macos_total_ram_mb().unwrap_or(0);

    Some(GpuInfo {
        name: format!("{} GPU (Metal, Unified Memory)", brand),
        vram_total_mb: total_ram_mb,
        vram_used_mb: 0,
        vram_usage_available: false,
        is_available: true,
    })
}

/// Get total system RAM on macOS in MB via sysctl hw.memsize
#[cfg(target_os = "macos")]
fn get_macos_total_ram_mb() -> Option<u64> {
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let bytes_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let bytes = bytes_str.parse::<u64>().ok()?;
    Some(bytes / 1024 / 1024)
}

// =============================================================================
// Windows GPU detection
// =============================================================================

#[cfg(target_os = "windows")]
fn detect_gpu_windows() -> GpuInfo {
    if let Some(info) = detect_gpu_nvidia_smi() {
        return info;
    }

    if let Some(info) = detect_gpu_wmic() {
        return info;
    }

    GpuInfo {
        name: "GPU not detected".to_string(),
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
