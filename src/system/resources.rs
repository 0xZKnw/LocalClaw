//! System resource monitoring
//!
//! Monitors RAM, VRAM, and other system resources during inference.

/// System resource usage
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
}

#[cfg(target_os = "windows")]
use std::process::Command;

/// Get system memory usage (best effort)
pub fn get_resource_usage() -> ResourceUsage {
    #[cfg(target_os = "windows")]
    {
        return get_resource_usage_windows();
    }

    #[cfg(not(target_os = "windows"))]
    {
        ResourceUsage::default()
    }
}

#[cfg(target_os = "windows")]
fn get_resource_usage_windows() -> ResourceUsage {
    let output = Command::new("wmic")
        .args(["OS", "get", "FreePhysicalMemory,TotalVisibleMemorySize", "/Value"])
        .output();

    let Ok(output) = output else {
        return ResourceUsage::default();
    };

    if !output.status.success() {
        return ResourceUsage::default();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut free_kb: Option<u64> = None;
    let mut total_kb: Option<u64> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("FreePhysicalMemory=") {
            let value = line.trim_start_matches("FreePhysicalMemory=").trim();
            if let Ok(parsed) = value.parse::<u64>() {
                free_kb = Some(parsed);
            }
        } else if line.starts_with("TotalVisibleMemorySize=") {
            let value = line.trim_start_matches("TotalVisibleMemorySize=").trim();
            if let Ok(parsed) = value.parse::<u64>() {
                total_kb = Some(parsed);
            }
        }
    }

    match (free_kb, total_kb) {
        (Some(free), Some(total)) if total > 0 => {
            let used_kb = total.saturating_sub(free);
            ResourceUsage {
                ram_used_mb: used_kb / 1024,
                ram_total_mb: total / 1024,
            }
        }
        _ => ResourceUsage::default(),
    }
}
