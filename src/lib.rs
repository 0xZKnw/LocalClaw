//! LocalClaw Library
//!
//! Core library for the LocalClaw desktop application.

pub mod agent;
pub mod app;
pub mod inference;
pub mod storage;
pub mod system;
pub mod types;
pub mod ui;

/// Safely truncate a string at a char boundary, never panics.
pub fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk backwards from max_bytes to find a valid char boundary
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
