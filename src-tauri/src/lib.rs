mod commands;
mod creem;
mod keychain;
mod poller;
mod sound;
mod state;
mod tray;

use std::sync::{Arc, Mutex};

use tauri::Manager;
use tokio::sync::Notify;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // single-instance must be registered first; desktop only.
    #[cfg(all(desktop, not(any(target_os = "android", target_os = "ios"))))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_settings(app);
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // macOS: behave as a menu-bar app with no dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();

            // Audio player thread + shared state.
            let sound_tx = sound::spawn_sound_player(handle.clone());
            app.manage(AppState {
                sound_tx: Mutex::new(sound_tx),
                wake: Arc::new(Notify::new()),
                tray: Mutex::new(None),
            });

            tray::build_tray(&handle)?;

            // Ask for notification permission once (no-op where pre-granted).
            #[cfg(desktop)]
            {
                use tauri_plugin_notification::NotificationExt;
                let _ = app.notification().request_permission();
            }

            // Keep the app alive in the tray when the settings window closes.
            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            tauri::async_runtime::spawn(poller::run_poller(handle.clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::clear_api_key,
            commands::test_connection,
            commands::play_test_sound,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
