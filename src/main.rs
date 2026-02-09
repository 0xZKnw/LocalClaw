//! LocalClaw - Local LLM Chat Application
//!
//! A desktop application for running local Large Language Models with a beautiful GUI.

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use localclaw::app::App;

fn main() {
    // Initialize tracing subscriber for logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("localclaw=info".parse().unwrap()))
        .init();

    info!("Starting LocalClaw v{}", env!("CARGO_PKG_VERSION"));

    // Initialize storage directory structure
    if let Err(e) = localclaw::storage::init_storage() {
        tracing::error!("Failed to initialize storage: {}", e);
    }

    // Launch Dioxus desktop application
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::default()
                .with_menu(None) // Remove the default menu bar
                .with_window(
                    WindowBuilder::new()
                        .with_title("LocalClaw")
                        .with_inner_size(LogicalSize::new(1200.0, 800.0)),
                ),
        )
        .launch(App);
}
