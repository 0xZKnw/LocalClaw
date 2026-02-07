use crate::app::AppState;
use crate::storage::settings::save_settings;
use crate::system::gpu::{detect_gpu, GpuInfo};
use crate::system::resources::{get_resource_usage, ResourceUsage};
use dioxus::prelude::*;
use std::process::Command;

pub fn HardwareSettings() -> Element {
    let app_state = use_context::<AppState>();
    let settings = app_state.settings.read().clone();
    let gpu_layers = settings.gpu_layers;
    let models_dir = settings.models_directory.to_string_lossy().to_string();
    let models_dir_path = settings.models_directory.clone();
    let auto_load_model = settings.auto_load_model;
    let last_model_path = settings.last_model_path.clone();
    let mut app_state_gpu_layers = app_state.clone();
    let mut app_state_auto_load = app_state.clone();

    let gpu_info = use_signal(GpuInfo::default);
    let ram_usage = use_signal(ResourceUsage::default);
    let info_loaded = use_signal(|| false);

    {
        let mut gpu_info = gpu_info.clone();
        let mut ram_usage = ram_usage.clone();
        let mut info_loaded = info_loaded.clone();
        use_effect(move || {
            if !info_loaded() {
                gpu_info.set(detect_gpu());
                ram_usage.set(get_resource_usage());
                info_loaded.set(true);
            }
        });
    }

    let gpu_snapshot = gpu_info.read().clone();
    let ram_snapshot = ram_usage.read().clone();

    let gpu_name = if gpu_snapshot.is_available && !gpu_snapshot.name.is_empty() {
        gpu_snapshot.name.clone()
    } else {
        "GPU non detecte".to_string()
    };

    let vram_total_mb = gpu_snapshot.vram_total_mb;
    let vram_used_mb = gpu_snapshot.vram_used_mb;
    let vram_usage_available = gpu_snapshot.vram_usage_available && vram_total_mb > 0;
    let vram_total_gb = vram_total_mb as f64 / 1024.0;
    let vram_used_gb = vram_used_mb as f64 / 1024.0;
    let vram_free_gb = vram_total_mb.saturating_sub(vram_used_mb) as f64 / 1024.0;
    let vram_percent = if vram_usage_available && vram_total_mb > 0 {
        (vram_used_mb as f64 / vram_total_mb as f64) * 100.0
    } else {
        0.0
    };

    let ram_total_mb = ram_snapshot.ram_total_mb;
    let ram_used_mb = ram_snapshot.ram_used_mb;
    let ram_free_mb = ram_total_mb.saturating_sub(ram_used_mb);
    let ram_total_gb = ram_total_mb as f64 / 1024.0;
    let ram_used_gb = ram_used_mb as f64 / 1024.0;
    let ram_free_gb = ram_free_mb as f64 / 1024.0;
    let ram_percent = if ram_total_mb > 0 {
        (ram_used_mb as f64 / ram_total_mb as f64) * 100.0
    } else {
        0.0
    };

    rsx! {
        div {
            class: "space-y-6 max-w-3xl mx-auto animate-fade-in-up pb-8",

            // GPU Info Card — glass
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                    "GPU Information"
                }

                div {
                    class: "flex items-start gap-4",

                    div {
                        class: "w-12 h-12 rounded-xl bg-[var(--accent-primary-10)] flex items-center justify-center text-[var(--accent-primary)]",
                        svg { class: "w-6 h-6", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "1.5",
                            path { d: "M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" }
                        }
                    }

                    div { class: "flex-1",
                        div { class: "font-semibold text-[var(--text-primary)]", "{gpu_name}" }

                        div { class: "mt-3 space-y-2",
                            if vram_total_mb == 0 {
                                p { class: "text-xs text-[var(--text-tertiary)]", "VRAM indisponible" }
                            } else if vram_usage_available {
                                div { class: "flex justify-between text-xs text-[var(--text-secondary)]",
                                    span { "VRAM utilisee" }
                                    span { class: "font-mono", "{vram_used_gb:.1} / {vram_total_gb:.1} GB" }
                                }
                                div { class: "flex justify-between text-xs text-[var(--text-secondary)]",
                                    span { "VRAM restante" }
                                    span { class: "font-mono", "{vram_free_gb:.1} GB" }
                                }
                                // Progress Bar — accent gradient
                                div {
                                    class: "w-full rounded-full h-1.5 overflow-hidden bg-white/[0.06]",
                                    div {
                                        class: "h-1.5 rounded-full transition-all",
                                        style: "width: {vram_percent}%; background: var(--accent-gradient);"
                                    }
                                }
                            } else {
                                div { class: "flex justify-between text-xs text-[var(--text-secondary)]",
                                    span { "VRAM totale" }
                                    span { class: "font-mono", "{vram_total_gb:.1} GB" }
                                }
                                p { class: "text-xs text-[var(--text-tertiary)]", "Utilisation VRAM indisponible" }
                            }
                        }
                    }
                }
            }

            // System Memory Card — glass
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                    "System Memory"
                }

                if ram_total_mb == 0 {
                    p { class: "text-xs text-[var(--text-tertiary)]", "RAM indisponible" }
                } else {
                    div { class: "space-y-2",
                        div { class: "flex justify-between text-xs text-[var(--text-secondary)]",
                            span { "RAM utilisee" }
                            span { class: "font-mono", "{ram_used_gb:.1} / {ram_total_gb:.1} GB" }
                        }
                        div { class: "flex justify-between text-xs text-[var(--text-secondary)]",
                            span { "RAM restante" }
                            span { class: "font-mono", "{ram_free_gb:.1} GB" }
                        }
                        div {
                            class: "w-full rounded-full h-1.5 overflow-hidden bg-white/[0.06]",
                            div {
                                class: "h-1.5 rounded-full transition-all",
                                style: "width: {ram_percent}%; background: var(--accent-gradient);"
                            }
                        }
                    }
                }
            }

            // Settings Card — glass
            div {
                class: "p-5 rounded-2xl glass-md",

                h3 {
                    class: "text-base font-semibold mb-5 text-[var(--text-primary)]",
                    "Hardware Acceleration"
                }

                // Auto-load Model Toggle
                div { class: "mb-6",
                    div { class: "flex items-center justify-between",
                        div {
                            label { class: "text-sm font-medium text-[var(--text-primary)]", "Charger auto. au demarrage" }
                            p { class: "text-xs text-[var(--text-tertiary)] mt-0.5",
                                {
                                    if let Some(ref path) = last_model_path {
                                        format!("Dernier: {}", std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
                                    } else {
                                        "Aucun modele sauvegarde".to_string()
                                    }
                                }
                            }
                        }
                        button {
                            class: if auto_load_model { "toggle-switch active" } else { "toggle-switch" },
                            onclick: move |_| {
                                let mut settings = app_state_auto_load.settings.write();
                                settings.auto_load_model = !settings.auto_load_model;
                                if let Err(error) = save_settings(&settings) {
                                    tracing::error!("Failed to save settings: {}", error);
                                }
                            },
                            div { class: "toggle-switch-knob" }
                        }
                    }
                }

                // GPU Layers Control
                div { class: "mb-6",
                    div { class: "flex justify-between items-center mb-2",
                        label { class: "text-sm font-medium text-[var(--text-primary)]", "GPU Layers" }
                        span {
                            class: "text-xs font-mono px-2 py-1 rounded-lg bg-white/[0.04] text-[var(--text-secondary)] border border-[var(--border-subtle)]",
                            "{gpu_layers}"
                        }
                    }
                    input {
                        r#type: "range",
                        min: "0",
                        max: "99",
                        value: "{gpu_layers}",
                        oninput: move |e| {
                            let value = e.value().parse().unwrap_or(0);
                            let mut settings = app_state_gpu_layers.settings.write();
                            settings.gpu_layers = value;
                            if let Err(error) = save_settings(&settings) {
                                tracing::error!("Failed to save settings: {}", error);
                            }
                        },
                        class: "w-full",
                    }
                    p { class: "text-xs text-[var(--text-tertiary)] mt-1.5",
                        "Layers to offload to GPU. Higher values need more VRAM."
                    }
                }

                // Models Directory Input
                div {
                    label { class: "text-sm font-medium text-[var(--text-primary)] mb-2 block", "Models Directory" }
                    div { class: "flex gap-2",
                        input {
                            r#type: "text",
                            readonly: true,
                            value: "{models_dir}",
                            class: "flex-1 py-2.5 px-3 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-secondary)] text-sm cursor-not-allowed",
                        }
                        button {
                            class: "px-4 py-2.5 rounded-xl bg-white/[0.04] border border-[var(--border-subtle)] text-[var(--text-primary)] text-sm font-medium hover:bg-white/[0.08] transition-colors",
                            onclick: move |_| {
                                let path = &models_dir_path;
                                let result = if cfg!(target_os = "windows") {
                                    Command::new("explorer").arg(path).spawn()
                                } else if cfg!(target_os = "macos") {
                                    Command::new("open").arg(path).spawn()
                                } else {
                                    Command::new("xdg-open").arg(path).spawn()
                                };

                                if let Err(error) = result {
                                    tracing::error!("Failed to open models directory: {}", error);
                                }
                            },
                            "Open"
                        }
                    }
                    p { class: "text-xs text-[var(--text-tertiary)] mt-1.5",
                        "Location where model files (.gguf) are stored."
                    }
                }
            }
        }
    }
}
