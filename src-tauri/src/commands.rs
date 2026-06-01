//! Tauri commands bridging the React settings window to Rust.
//! The API key is write-only from JS: it goes into the keychain and is never
//! returned.

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

use crate::creem::{CreemClient, CreemError};
use crate::state::AppState;
use crate::{keychain, poller};

#[derive(Serialize)]
pub struct SettingsDto {
    pub test_mode: bool,
    pub interval_secs: u64,
    pub autostart: bool,
    pub has_api_key: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> SettingsDto {
    let (test_mode, interval_secs) = poller::read_config(&app);
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    SettingsDto {
        test_mode,
        interval_secs,
        autostart,
        has_api_key: keychain::get_api_key().is_some(),
    }
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    test_mode: bool,
    interval_secs: u64,
    autostart: bool,
) -> Result<(), String> {
    let (prev_mode, _) = poller::read_config(&app);

    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("test_mode", json!(test_mode));
    store.set("interval_secs", json!(interval_secs.max(15)));
    store.save().map_err(|e| e.to_string())?;

    let launcher = app.autolaunch();
    let _ = if autostart {
        launcher.enable()
    } else {
        launcher.disable()
    };

    // Switching test/prod changes the transaction universe — re-baseline.
    if prev_mode != test_mode {
        poller::reset_baseline(&app);
    }
    app.state::<AppState>().wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn set_api_key(app: AppHandle, api_key: String) -> Result<(), String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("API key is empty".into());
    }
    keychain::set_api_key(key)?;
    poller::reset_baseline(&app);
    app.state::<AppState>().wake.notify_one();
    Ok(())
}

/// Plays the notification sound once, for testing from Settings.
#[tauri::command]
pub fn play_test_sound(app: AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.play_sound();
    }
}

#[tauri::command]
pub fn clear_api_key(app: AppHandle) -> Result<(), String> {
    keychain::clear_api_key()?;
    poller::reset_baseline(&app);
    app.state::<AppState>().wake.notify_one();
    Ok(())
}

/// Validates a key by performing one authenticated request, using the exact
/// mode currently shown in the UI (not the persisted setting). If `api_key`
/// is provided it is tested directly; otherwise the stored key is used.
#[tauri::command]
pub async fn test_connection(
    api_key: Option<String>,
    test_mode: bool,
) -> Result<usize, String> {
    let key = match api_key {
        Some(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => keychain::get_api_key().ok_or("No API key set")?,
    };
    let client = CreemClient::new(key, test_mode);
    match client.search_transactions(1).await {
        Ok(txs) => Ok(txs.len()),
        Err(CreemError::Unauthorized) => Err("Invalid API key".into()),
        Err(e) => Err(e.to_string()),
    }
}
