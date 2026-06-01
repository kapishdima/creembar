use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

use crate::tray::TrayHandles;

/// Shared application state, registered via `app.manage(...)`.
pub struct AppState {
    /// Signals the SoundPlayer thread to play the cha-ching once.
    pub sound_tx: Mutex<std::sync::mpsc::Sender<()>>,
    /// Wakes the poller immediately (e.g. after a settings/key change).
    pub wake: Arc<Notify>,
    /// Tray menu item handles, set once the tray is built.
    pub tray: Mutex<Option<TrayHandles>>,
}

impl AppState {
    pub fn play_sound(&self) {
        if let Ok(tx) = self.sound_tx.lock() {
            let _ = tx.send(());
        }
    }
}
