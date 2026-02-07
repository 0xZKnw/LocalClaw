use dioxus::prelude::*;
use crate::app::{AppState, ModelState};
use crate::storage::huggingface::download_model;
use crate::storage::models::scan_models_directory;
use crate::ui::components::loading::Spinner;


#[component]
pub fn ModelPicker() -> Element {
    let app_state = use_context::<AppState>();
    let models_directory = app_state.settings.read().models_directory.clone();
    
    let mut models = use_signal(Vec::new);
    let mut selected_model_path = use_signal(|| None::<String>);
    let mut dropdown_open = use_signal(|| false);
    
    // Download dialog state
    let mut show_download_dialog = use_signal(|| false);
    let mut download_url = use_signal(|| String::new());
    let mut is_downloading = use_signal(|| false);
    let mut download_error = use_signal(|| None::<String>);
    let mut download_success = use_signal(|| false);
    
    let models_directory_clone = models_directory.clone();
    use_effect(move || {
        let found_models = scan_models_directory(&models_directory_clone).unwrap_or_default();
        if selected_model_path.read().is_none() {
            if let Some(first_model) = found_models.first() {
                let path_str = first_model.path.to_string_lossy().to_string();
                tracing::debug!("Pre-selecting first model: {}", path_str);
                selected_model_path.set(Some(path_str));
            }
        }
        models.set(found_models);
    });

    // Handlers
    let app_state_for_load = app_state.clone();
    let selected_model_path_for_load = selected_model_path.clone();
    let handle_load = move |_| {
        let mut app_state = app_state_for_load.clone();
        app_state.model_state.set(ModelState::Loading);
        let path = selected_model_path_for_load
            .read()
            .clone()
            .unwrap_or_default();
        let gpu_layers = app_state.settings.read().gpu_layers;
        spawn(async move {
            let result = {
                let mut engine = app_state.engine.lock().await;
                if !engine.is_initialized() {
                    if let Err(e) = engine.init() {
                        return app_state.model_state.set(ModelState::Error(e.to_string()));
                    }
                }
                engine.load_model(&path, gpu_layers)
            };
            match result {
                Ok(_info) => app_state.model_state.set(ModelState::Loaded(path)),
                Err(e) => app_state.model_state.set(ModelState::Error(e.to_string())),
            }
        });
    };

    let app_state_for_unload = app_state.clone();
    let handle_unload = move |_| {
        let mut app_state = app_state_for_unload.clone();
        spawn(async move {
            let mut engine = app_state.engine.lock().await;
            engine.unload_model();
        });
        app_state.model_state.set(ModelState::NotLoaded);
    };

    let app_state_for_refresh = app_state.clone();
    let mut models_for_refresh = models.clone();
    let handle_refresh = move |_| {
        let models_directory = app_state_for_refresh
            .settings
            .read()
            .models_directory
            .clone();
        models_for_refresh.set(scan_models_directory(&models_directory).unwrap_or_default());
    };

    // Download handler
    let handle_download = move |_| {
        let url = download_url.read().clone();
        if url.is_empty() {
            download_error.set(Some("Please enter a URL".to_string()));
            return;
        }
        
        is_downloading.set(true);
        download_error.set(None);
        download_success.set(false);
        
        let mut is_downloading_inner = is_downloading.clone();
        let mut download_error_inner = download_error.clone();
        let mut download_success_inner = download_success.clone();
        let mut models_inner = models.clone();
        let models_directory_inner = models_directory.clone();
        let mut download_url_inner = download_url.clone();
        
        spawn(async move {
            let result = download_model(&url, |_downloaded, _total| {
            }).await;
            
            is_downloading_inner.set(false);
            
            match result {
                Ok(path) => {
                    tracing::info!("Downloaded model to: {:?}", path);
                    download_success_inner.set(true);
                    let found_models = scan_models_directory(&models_directory_inner).unwrap_or_default();
                    models_inner.set(found_models);
                    download_url_inner.set(String::new());
                }
                Err(e) => {
                    tracing::error!("Download failed: {}", e);
                    download_error_inner.set(Some(e));
                }
            }
        });
    };

    rsx! {
        div {
            class: "flex flex-col gap-3",
            
            // Header with Refresh
            div {
                class: "flex items-center justify-between",
                span {
                    class: "text-[10px] uppercase tracking-widest text-[var(--text-tertiary)] font-semibold select-none",
                    if app_state.settings.read().language == "en" { "Active Model" } else { "Modele actif" }
                }
                button {
                    onclick: handle_refresh,
                    class: "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors p-1 rounded-md hover:bg-white/[0.06]",
                    title: if app_state.settings.read().language == "en" { "Rescan models" } else { "Re-scanner les modeles" },
                    svg {
                        class: "w-3 h-3",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M23 4v6h-6" }
                        path { d: "M1 20v-6h6" }
                        path { d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" }
                    }
                }
            }

            if models.read().is_empty() {
                div {
                    class: "flex flex-col items-center justify-center p-4 border border-dashed border-[var(--border-subtle)] rounded-xl gap-2",
                    span { class: "text-sm text-[var(--text-secondary)] font-medium",
                        if app_state.settings.read().language == "en" { "No models found" } else { "Aucun modele trouve" }
                    }
                    span { class: "text-[10px] text-[var(--text-tertiary)] text-center",
                        if app_state.settings.read().language == "en" { "Place .gguf files in /models" } else { "Placez des fichiers .gguf dans /models" }
                    }
                }
            } else {
                div {
                    class: "flex flex-col gap-2",
                    
                    // Model Selector — custom dropdown
                    {
                        let is_disabled = matches!(*app_state.model_state.read(), ModelState::Loading | ModelState::Loaded(_));
                        let selected_name = {
                            let sel = selected_model_path.read();
                            let mods = models.read();
                            let fallback = if app_state.settings.read().language == "en" { "Select a model" } else { "Choisir un modele" };
                            sel.as_ref().and_then(|p| mods.iter().find(|m| m.path.to_string_lossy() == *p).map(|m| m.filename.clone())).unwrap_or_else(|| fallback.to_string())
                        };

                        rsx! {
                            div {
                                class: "relative",

                                // Trigger button
                                button {
                                    r#type: "button",
                                    disabled: is_disabled,
                                    onclick: move |_| if !is_disabled { dropdown_open.set(!dropdown_open()) },
                                    class: "w-full flex items-center justify-between gap-2 py-2.5 px-3 rounded-xl text-sm font-medium transition-all cursor-pointer",
                                    style: "background: var(--bg-tertiary); border: 1px solid var(--border-subtle);",
                                    onmouseover: move |_| {},

                                    span {
                                        class: "truncate text-[var(--text-primary)]",
                                        "{selected_name}"
                                    }
                                    svg {
                                        class: if dropdown_open() { "w-4 h-4 text-[var(--text-tertiary)] transition-transform rotate-180" } else { "w-4 h-4 text-[var(--text-tertiary)] transition-transform" },
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        polyline { points: "6 9 12 15 18 9" }
                                    }
                                }

                                // Dropdown panel
                                if dropdown_open() {
                                    div {
                                        class: "absolute left-0 right-0 mt-1 rounded-xl overflow-hidden z-50 animate-fade-in",
                                        style: "background: var(--bg-elevated); border: 1px solid var(--border-medium); box-shadow: 0 8px 24px -4px rgba(30,25,20,0.3);",

                                        div {
                                            class: "max-h-48 overflow-y-auto custom-scrollbar py-1",

                                            for model in models.read().iter() {
                                                {
                                                    let path_str = model.path.to_string_lossy().to_string();
                                                    let is_selected = selected_model_path.read().as_ref().map_or(false, |p| *p == path_str);
                                                    let filename = model.filename.clone();
                                                    let size = model.size_string();

                                                    rsx! {
                                                        button {
                                                            r#type: "button",
                                                            onclick: {
                                                                let path_str = path_str.clone();
                                                                move |_| {
                                                                    selected_model_path.set(Some(path_str.clone()));
                                                                    dropdown_open.set(false);
                                                                }
                                                            },
                                                            class: if is_selected {
                                                                "w-full flex items-center justify-between px-3 py-2 text-left text-sm transition-all"
                                                            } else {
                                                                "w-full flex items-center justify-between px-3 py-2 text-left text-sm transition-all"
                                                            },
                                                            style: if is_selected {
                                                                "background: var(--accent-soft); color: var(--accent-primary);"
                                                            } else {
                                                                "color: var(--text-primary);"
                                                            },

                                                            span { class: "truncate font-medium", "{filename}" }
                                                            span {
                                                                class: "flex-shrink-0 text-[10px] font-mono text-[var(--text-tertiary)] ml-2",
                                                                "{size}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Size badge
                    if let Some(path) = selected_model_path.read().as_ref() {
                        if let Some(model) = models.read().iter().find(|m| m.path.to_string_lossy() == *path) {
                            div {
                                class: "flex justify-end",
                                span {
                                    class: "px-2 py-0.5 rounded-md text-[10px] font-mono bg-white/[0.03] text-[var(--text-tertiary)] border border-[var(--border-subtle)]",
                                    "{model.size_string()}"
                                }
                            }
                        }
                    }

                    // Actions & Status
                    match *app_state.model_state.read() {
                        ModelState::NotLoaded => rsx! {
                            button {
                                onclick: handle_load,
                                class: "w-full flex items-center justify-center gap-2 bg-white/[0.03] border border-[var(--border-subtle)] hover:border-[var(--accent-primary)] hover:text-[var(--accent-primary)] text-[var(--text-secondary)] text-sm font-medium py-2.5 rounded-xl transition-all active:scale-[0.98]",
                                svg {
                                    class: "w-4 h-4",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M5 12h14" }
                                    path { d: "M12 5l7 7-7 7" }
                                }
                                if app_state.settings.read().language == "en" { "Load Model" } else { "Charger le modele" }
                            }
                        },
                        ModelState::Loading => rsx! {
                            div {
                                class: "w-full flex flex-col gap-2 bg-white/[0.03] border border-[var(--border-subtle)] p-3 rounded-xl",
                                div {
                                    class: "flex items-center gap-2",
                                    Spinner { size: 14 }
                                    span { class: "text-xs font-medium text-[var(--text-secondary)]",
                                        if app_state.settings.read().language == "en" { "Loading into memory..." } else { "Chargement en memoire..." }
                                    }
                                }
                                div { class: "loading-bar" }
                            }
                        },
                        ModelState::Loaded(_) => rsx! {
                            div {
                                class: "flex items-center gap-2",
                                div {
                                    class: "flex-1 flex items-center gap-2 px-3 py-2 bg-[var(--bg-success-subtle)] border border-[var(--border-success-subtle)] rounded-xl",
                                    div { class: "status-dot status-dot-ready" }
                                    span { class: "text-xs font-medium text-[var(--text-success)]",
                                        if app_state.settings.read().language == "en" { "Ready" } else { "Pret" }
                                    }
                                }
                                button {
                                    onclick: handle_unload,
                                    class: "px-3 py-2 text-sm text-[var(--text-secondary)] border border-[var(--border-subtle)] rounded-xl hover:bg-[var(--bg-error-subtle)] hover:border-[var(--border-error-subtle)] hover:text-[var(--text-error)] transition-colors",
                                    title: if app_state.settings.read().language == "en" { "Unload Model" } else { "Decharger le modele" },
                                    svg {
                                        class: "w-4 h-4",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path { d: "M18.36 6.64a9 9 0 1 1-12.73 0" }
                                        line { x1: "12", y1: "2", x2: "12", y2: "12" }
                                    }
                                }
                            }
                        },
                        ModelState::Error(ref msg) => rsx! {
                            div {
                                class: "w-full p-2 bg-[var(--bg-error-subtle)] border border-[var(--border-error-subtle)] rounded-xl text-xs text-[var(--text-error)]",
                                "{msg}"
                            }
                        }
                    }
                }
            }

            // Download from HuggingFace
            div {
                class: "divider-premium"
            }
            button {
                onclick: move |_| show_download_dialog.set(true),
                class: "w-full flex items-center justify-center gap-2 text-[var(--text-tertiary)] hover:text-[var(--accent-primary)] text-xs font-medium py-1.5 rounded-lg transition-colors",
                disabled: *is_downloading.read(),
                svg {
                    class: "w-3.5 h-3.5",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                    polyline { points: "7 10 12 15 17 10" }
                    line { x1: "12", y1: "15", x2: "12", y2: "3" }
                }
                if app_state.settings.read().language == "en" { "Download from HuggingFace" } else { "Telecharger depuis HuggingFace" }
            }

            // Download Dialog
            if *show_download_dialog.read() {
                div {
                    class: "fixed inset-0 bg-black/60 backdrop-blur-xl z-50 flex items-center justify-center p-4",
                    onclick: move |_| show_download_dialog.set(false),
                    
                    div {
                        class: "w-full max-w-md glass-strong rounded-2xl p-6 animate-scale-in",
                        onclick: move |e| e.stop_propagation(),
                        
                        h3 {
                            class: "text-lg font-semibold text-[var(--text-primary)] mb-2",
                            if app_state.settings.read().language == "en" { "Download Model from HuggingFace" } else { "Telecharger un modele HuggingFace" }
                        }
                        
                        p {
                            class: "text-sm text-[var(--text-secondary)] mb-4",
                            if app_state.settings.read().language == "en" { "Enter a HuggingFace repository URL or model ID. Example: TheBloke/Llama-2-7B-GGUF" } else { "Entrez une URL de depot HuggingFace ou un ID de modele. Exemple : TheBloke/Llama-2-7B-GGUF" }
                        }
                        
                        input {
                            r#type: "text",
                            value: "{download_url.read()}",
                            oninput: move |e| download_url.set(e.value()),
                            disabled: *is_downloading.read(),
                            placeholder: "username/repo or full URL",
                            class: "w-full p-3 rounded-xl bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:border-[var(--accent-primary)] transition-all outline-none mb-4",
                        }
                        
                        if *is_downloading.read() {
                            div {
                                class: "mb-4 flex items-center justify-center gap-3 p-3 bg-white/[0.02] rounded-xl border border-[var(--border-subtle)]",
                                Spinner { size: 16 }
                                span { class: "text-sm text-[var(--text-secondary)]",
                                    if app_state.settings.read().language == "en" { "Downloading..." } else { "Telechargement..." }
                                }
                            }
                        }
                        
                        if let Some(error) = download_error.read().as_ref() {
                            div {
                                class: "p-3 mb-4 bg-[var(--bg-error-subtle)] border border-[var(--border-error-subtle)] rounded-xl text-xs text-[var(--text-error)]",
                                "{error}"
                            }
                        }
                        
                        if *download_success.read() {
                            div {
                                class: "p-3 mb-4 bg-[var(--bg-success-subtle)] border border-[var(--border-success-subtle)] rounded-xl text-xs text-[var(--text-success)]",
                                if app_state.settings.read().language == "en" { "Download complete! Model is now available in the list." } else { "Telechargement termine ! Le modele est maintenant disponible." }
                            }
                        }
                        
                        div {
                            class: "flex gap-3",
                            button {
                                onclick: move |_| show_download_dialog.set(false),
                                class: "btn-ghost flex-1",
                                if app_state.settings.read().language == "en" { "Cancel" } else { "Annuler" }
                            }
                            button {
                                onclick: handle_download,
                                disabled: *is_downloading.read(),
                                class: "btn-primary flex-1 flex items-center justify-center gap-2",
                                if *is_downloading.read() {
                                    Spinner { size: 14 }
                                    if app_state.settings.read().language == "en" { "Downloading..." } else { "Telechargement..." }
                                } else {
                                    if app_state.settings.read().language == "en" { "Download" } else { "Telecharger" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
