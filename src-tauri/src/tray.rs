//! System tray icon + menu, and runtime updates to the menu text.

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::state::AppState;

/// Handles to the dynamic menu items, kept so the poller can relabel them.
pub struct TrayHandles {
    pub header: MenuItem<Wry>,
    pub recent: Vec<MenuItem<Wry>>,
}

pub const RECENT_SLOTS: usize = 5;

/// Builds the tray icon and menu, and stores the item handles in `AppState`.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let header = MenuItemBuilder::with_id("header", "Starting…")
        .enabled(false)
        .build(app)?;

    let mut recent = Vec::with_capacity(RECENT_SLOTS);
    for i in 0..RECENT_SLOTS {
        recent.push(
            MenuItemBuilder::with_id(format!("recent_{i}"), "—")
                .enabled(false)
                .build(app)?,
        );
    }

    let settings = MenuItemBuilder::with_id("settings", "Settings…").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit creembar").build(app)?;

    let mut builder = MenuBuilder::new(app).item(&header).separator();
    for item in &recent {
        builder = builder.item(item);
    }
    let menu = builder.separator().item(&settings).item(&quit).build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon is bundled");

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    if let Some(state) = app.try_state::<AppState>() {
        *state.tray.lock().unwrap() = Some(TrayHandles { header, recent });
    }

    Ok(())
}

/// Shows and focuses the settings window (created hidden at startup).
pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Sets the header line (totals or a status message).
pub fn set_header(app: &AppHandle, text: String) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        if let Some(state) = app.try_state::<AppState>() {
            if let Some(handles) = state.tray.lock().unwrap().as_ref() {
                let _ = handles.header.set_text(&text);
            }
        }
    });
}

/// Replaces the header and the recent-payment lines in one main-thread hop.
pub fn update_tray(app: &AppHandle, header: String, recent: Vec<String>) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        if let Some(state) = app.try_state::<AppState>() {
            if let Some(handles) = state.tray.lock().unwrap().as_ref() {
                let _ = handles.header.set_text(&header);
                for (i, item) in handles.recent.iter().enumerate() {
                    let label = recent.get(i).map(String::as_str).unwrap_or("—");
                    let _ = item.set_text(label);
                }
            }
        }
    });
}
