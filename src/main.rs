//! LocaLM - Local LLM Chat Application
//!
//! A desktop application for running local Large Language Models with a beautiful GUI.

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod app;
mod inference;
mod storage;
mod system;
mod types;
mod ui;

use app::App;

fn main() {
    // Initialize tracing subscriber for logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("localm=info".parse().unwrap()))
        .init();

    info!("Starting LocaLM v{}", env!("CARGO_PKG_VERSION"));

    // Launch Dioxus desktop application
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::default().with_window(
                WindowBuilder::new()
                    .with_title("LocaLM")
                    .with_inner_size(LogicalSize::new(1200.0, 800.0)),
            ),
        )
        .launch(App);
}
