//! Custom "cha-ching" playback.
//!
//! A tray-only app may have no live webview, so we play audio in Rust.
//! `rodio::OutputStream` is `!Send`, so a dedicated thread owns the audio
//! device for the whole process lifetime and plays on each channel signal.

use std::io::Cursor;
use std::sync::mpsc::{channel, Sender};

use tauri::{AppHandle, Manager};

/// Spawns the audio thread and returns the sender used to trigger playback.
pub fn spawn_sound_player(app: AppHandle) -> Sender<()> {
    let (tx, rx) = channel::<()>();

    std::thread::spawn(move || {
        // Resolve and load the bundled sound once.
        let bytes = app
            .path()
            .resolve("sounds/chaching.mp3", tauri::path::BaseDirectory::Resource)
            .ok()
            .and_then(|p| std::fs::read(p).ok());

        // Keep the output stream alive for the thread's lifetime.
        let (_stream, handle) = match rodio::OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[sound] no audio output device: {e}");
                return;
            }
        };

        for _ in rx {
            let Some(data) = bytes.clone() else { continue };
            match rodio::Sink::try_new(&handle) {
                Ok(sink) => match rodio::Decoder::new(Cursor::new(data)) {
                    Ok(decoder) => {
                        sink.append(decoder);
                        sink.detach();
                    }
                    Err(e) => eprintln!("[sound] decode failed: {e}"),
                },
                Err(e) => eprintln!("[sound] sink failed: {e}"),
            }
        }
    });

    tx
}
