#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::{AppHandle, State};
use wtch::config::Config;
use wtch::preset::resolve_preset;
use wtch::scheduler::{start_scheduler, SharedConfig};
use wtch::state::{new_shared_state, AppState, SharedState};
use wtch::tray::setup_tray;
use wtch::watcher::evaluate_watch;

/// Serializable preset info returned to the frontend.
#[derive(Serialize)]
struct PresetInfo {
    name: String,
    enabled: bool,
}

/// Returns the current application state as a JSON-serializable value.
#[tauri::command]
async fn get_state(state: State<'_, SharedState>) -> Result<AppState, String> {
    let guard = state.read().await;
    Ok(guard.clone())
}

/// Triggers an immediate re-evaluation of all watches and updates the state.
#[tauri::command]
async fn refresh(
    state: State<'_, SharedState>,
    config: State<'_, SharedConfig>,
) -> Result<(), String> {
    let config = config.read().await.clone();
    let state = state.inner().clone();

    wtch::logging::info(&format!("[refresh] manual refresh: {} watches", config.watches.len()));
    let mut results = Vec::new();
    for watch in &config.watches {
        let watch_state = evaluate_watch(watch, &config).await;
        results.push(watch_state);
    }

    let mut guard = state.write().await;
    guard.watches = results;
    guard.last_check = Some(chrono::Utc::now().to_rfc3339());
    guard.groups = config.groups.clone();
    wtch::logging::info("[refresh] complete");

    Ok(())
}

/// Returns all known presets with a flag indicating whether each is currently enabled.
#[tauri::command]
async fn list_presets(config: State<'_, SharedConfig>) -> Result<Vec<PresetInfo>, String> {
    let all = wtch::preset::list_all_presets();
    let guard = config.read().await;
    let presets = all
        .into_iter()
        .map(|name| {
            let enabled = guard
                .watches
                .iter()
                .any(|w| w.preset.as_deref() == Some(&name));
            PresetInfo { name, enabled }
        })
        .collect();
    Ok(presets)
}

/// Enables or disables a preset by writing to / removing from the config file.
///
/// Does not hot-reload the running config; call `reload_config` afterwards if
/// you want the change to take effect immediately.
#[tauri::command]
fn toggle_preset(name: String, enabled: bool) -> Result<(), String> {
    if enabled {
        wtch::config::add_preset_to_file(&name).map_err(|e| e.to_string())
    } else {
        wtch::config::remove_preset_from_file(&name).map_err(|e| e.to_string())
    }
}

/// Rewrites the config file with `[[watch]]` blocks sorted according to `order`.
#[tauri::command]
fn reorder_watches(order: Vec<String>) -> Result<(), String> {
    wtch::config::reorder_watches_in_file(&order).map_err(|e| e.to_string())
}

/// Opens the config file in the system's default text editor.
#[tauri::command]
fn open_config() -> Result<(), String> {
    let path = wtch::config::config_path().ok_or("home directory not found")?;
    open::that(path).map_err(|e| e.to_string())
}

/// Returns the current debug mode status.
#[tauri::command]
async fn get_debug(config: State<'_, SharedConfig>) -> Result<bool, String> {
    Ok(config.read().await.general.debug)
}

/// Toggles debug logging. Writes `debug: true/false` to the config file.
#[tauri::command]
async fn set_debug(enabled: bool, config: State<'_, SharedConfig>) -> Result<(), String> {
    {
        let mut cfg = config.write().await;
        cfg.general.debug = enabled;
    }
    wtch::logging::init(enabled);

    let path = wtch::config::config_path().ok_or("home directory not found")?;
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(&content).unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

    if let Some(mapping) = doc.as_mapping_mut() {
        let general_key = serde_yaml::Value::String("general".to_string());
        let general = mapping
            .entry(general_key)
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        if let Some(gen_map) = general.as_mapping_mut() {
            gen_map.insert(
                serde_yaml::Value::String("debug".to_string()),
                serde_yaml::Value::Bool(enabled),
            );
        }
    }

    let output = serde_yaml::to_string(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, output).map_err(|e| e.to_string())
}

/// Opens the config directory in the system's file explorer.
#[tauri::command]
fn open_config_dir() -> Result<(), String> {
    let path = wtch::config::config_path().ok_or("home directory not found")?;
    let dir = path.parent().ok_or("config has no parent directory")?;
    open::that(dir).map_err(|e| e.to_string())
}

/// Reloads the config from disk, re-resolves presets, and re-evaluates all watches.
#[tauri::command]
async fn reload_config(
    shared_config: State<'_, SharedConfig>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut new_config = Config::load().map_err(|e| e.to_string())?;
    new_config.watches = new_config
        .watches
        .iter()
        .map(|w| resolve_preset(w))
        .collect();

    let mut results = Vec::new();
    for watch in &new_config.watches {
        let watch_state = evaluate_watch(watch, &new_config).await;
        results.push(watch_state);
    }

    {
        let mut cfg_guard = shared_config.write().await;
        *cfg_guard = new_config;
    }

    let groups = {
        let cfg = shared_config.read().await;
        cfg.groups.clone()
    };

    let mut state_guard = state.write().await;
    state_guard.watches = results;
    state_guard.last_check = Some(chrono::Utc::now().to_rfc3339());
    state_guard.groups = groups;

    Ok(())
}

/// Exits the application.
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn main() {
    let mut config = Config::load().unwrap_or_default();
    wtch::logging::init(config.general.debug);

    config.watches = config
        .watches
        .iter()
        .map(|w| resolve_preset(w))
        .collect();

    let shared_config: SharedConfig = std::sync::Arc::new(tokio::sync::RwLock::new(config));
    let shared_state = new_shared_state();

    let scheduler_state = shared_state.clone();
    let scheduler_config = shared_config.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(shared_state)
        .manage(shared_config)
        .setup(move |app| {
            setup_tray(app.handle())?;
            start_scheduler(scheduler_config, scheduler_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh,
            list_presets,
            toggle_preset,
            reorder_watches,
            open_config,
            open_config_dir,
            get_debug,
            set_debug,
            reload_config,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
