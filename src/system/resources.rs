//! System resource monitoring
//!
//! Monitors RAM, VRAM, and other system resources during inference.

/// System resource usage
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

/// Get system memory usage (best effort)
pub fn get_resource_usage() -> ResourceUsage {
    #[cfg(target_os = "windows")]
    {
        return get_resource_usage_windows();
    }

    #[cfg(target_os = "macos")]
    {
        return get_resource_usage_macos();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        ResourceUsage::default()
    }
}

// =============================================================================
// macOS resource monitoring
// =============================================================================

#[cfg(target_os = "macos")]
fn get_resource_usage_macos() -> ResourceUsage {
    let total_mb = get_macos_total_ram_mb().unwrap_or(0);
    let used_mb = get_macos_used_ram_mb().unwrap_or(0);

    ResourceUsage {
        ram_used_mb: used_mb,
        ram_total_mb: total_mb,
    }
}

/// Get total RAM via sysctl hw.memsize (returns bytes, we convert to MB)
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

/// Get used RAM via vm_stat (active + wired pages × page size)
#[cfg(target_os = "macos")]
fn get_macos_used_ram_mb() -> Option<u64> {
    let output = Command::new("vm_stat")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut page_size: u64 = 16384; // default for Apple Silicon
    let mut active_pages: u64 = 0;
    let mut wired_pages: u64 = 0;

    for line in stdout.lines() {
        // First line: "Mach Virtual Memory Statistics: (page size of 16384 bytes)"
        if line.contains("page size of") {
            if let Some(start) = line.find("page size of ") {
                let after = &line[start + 13..];
                if let Some(end) = after.find(' ') {
                    if let Ok(ps) = after[..end].parse::<u64>() {
                        page_size = ps;
                    }
                }
            }
        }

        // "Pages active:    123456."
        if line.starts_with("Pages active:") {
            let val = line.trim_start_matches("Pages active:")
                .trim()
                .trim_end_matches('.');
            if let Ok(v) = val.parse::<u64>() {
                active_pages = v;
            }
        }

        // "Pages wired down:    123456."
        if line.starts_with("Pages wired down:") {
            let val = line.trim_start_matches("Pages wired down:")
                .trim()
                .trim_end_matches('.');
            if let Ok(v) = val.parse::<u64>() {
                wired_pages = v;
            }
        }
    }

    let used_bytes = (active_pages + wired_pages) * page_size;
    Some(used_bytes / 1024 / 1024)
}

// =============================================================================
// Windows resource monitoring
// =============================================================================

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

