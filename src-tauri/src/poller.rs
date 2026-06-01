//! Background polling loop: fetch creem transactions, detect new paid ones,
//! notify (sound + native notification), and keep the tray totals fresh.

use std::collections::HashSet;
use std::time::Duration;

use chrono::{Local, TimeZone};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_store::StoreExt;
use tokio::sync::Notify;

use crate::creem::{CreemClient, CreemError, Transaction};
use crate::state::AppState;
use crate::{keychain, tray};

const SETTINGS_STORE: &str = "settings.json";
const STATE_STORE: &str = "state.json";
const PAGE_SIZE: u32 = 50;
const SEEN_CAP: usize = 2000;
const MIN_INTERVAL: u64 = 15;
const MAX_BACKOFF: u64 = 300;

pub fn default_interval() -> u64 {
    60
}

/// Reads the user settings mirror: (test_mode, interval_secs).
pub fn read_config(app: &AppHandle) -> (bool, u64) {
    let store = app.store(SETTINGS_STORE).ok();
    let test_mode = store
        .as_ref()
        .and_then(|s| s.get("test_mode"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true); // default to safe test mode until configured
    let interval = store
        .as_ref()
        .and_then(|s| s.get("interval_secs"))
        .and_then(|v| v.as_u64())
        .unwrap_or_else(default_interval)
        .max(MIN_INTERVAL);
    (test_mode, interval)
}

/// Clears the dedup baseline so the next poll re-baselines silently.
/// Called when the API key or test/prod mode changes.
pub fn reset_baseline(app: &AppHandle) {
    if let Ok(store) = app.store(STATE_STORE) {
        store.set("baselined", json!(false));
        store.set("seen_ids", json!(Vec::<String>::new()));
        let _ = store.save();
    }
}

pub async fn run_poller(app: AppHandle) {
    let wake = app.state::<AppState>().wake.clone();

    loop {
        let (test_mode, interval) = read_config(&app);

        let Some(key) = keychain::get_api_key() else {
            tray::set_header(&app, "Not configured — open Settings".into());
            wait(&wake, 30).await;
            continue;
        };

        let client = CreemClient::new(key, test_mode);
        match client.search_transactions(PAGE_SIZE).await {
            Ok(txs) => {
                handle_transactions(&app, txs);
                wait(&wake, interval).await;
            }
            Err(CreemError::Unauthorized) => {
                tray::set_header(&app, "Invalid API key".into());
                // Don't hammer; resume promptly when the key changes (wake).
                wait(&wake, 60).await;
            }
            Err(CreemError::RateLimited(retry)) => {
                let secs = retry.unwrap_or(interval).clamp(interval, MAX_BACKOFF);
                tray::set_header(&app, "Rate limited — backing off".into());
                wait(&wake, secs).await;
            }
            Err(CreemError::Network(e)) | Err(CreemError::Decode(e)) => {
                eprintln!("[poller] {e}");
                tray::set_header(&app, "Connection error — retrying".into());
                wait(&wake, interval.max(30)).await;
            }
        }
    }
}

/// Sleeps for `secs`, or returns early if woken by a settings/key change.
async fn wait(wake: &Notify, secs: u64) {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
        _ = wake.notified() => {}
    }
}

fn handle_transactions(app: &AppHandle, txs: Vec<Transaction>) {
    let Ok(store) = app.store(STATE_STORE) else {
        return;
    };

    let baselined = store
        .get("baselined")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut seen: HashSet<String> = store
        .get("seen_ids")
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut paid: Vec<&Transaction> = txs
        .iter()
        .filter(|t| t.status.eq_ignore_ascii_case("paid") && !t.id.is_empty())
        .collect();
    // Oldest first so notifications fire in chronological order.
    paid.sort_by_key(|t| t.created_at);

    if !baselined {
        // First successful poll: record everything as seen, notify for none.
        for t in &paid {
            seen.insert(t.id.clone());
        }
        store.set("baselined", json!(true));
    } else {
        for t in &paid {
            if seen.insert(t.id.clone()) {
                notify_payment(app, t);
            }
        }
    }

    persist_seen(&store, &paid, &mut seen);
    let _ = store.save();

    update_tray_totals(app, &paid);
}

fn persist_seen(
    store: &std::sync::Arc<tauri_plugin_store::Store<tauri::Wry>>,
    paid: &[&Transaction],
    seen: &mut HashSet<String>,
) {
    // Bound growth: if oversized, keep the most recent ids we know about.
    if seen.len() > SEEN_CAP {
        let mut recent_ids: Vec<(i64, String)> =
            paid.iter().map(|t| (t.created_at, t.id.clone())).collect();
        recent_ids.sort_by(|a, b| b.0.cmp(&a.0));
        let keep: HashSet<String> = recent_ids
            .into_iter()
            .take(SEEN_CAP)
            .map(|(_, id)| id)
            .collect();
        *seen = keep;
    }
    let ids: Vec<&String> = seen.iter().collect();
    store.set("seen_ids", json!(ids));
}

fn notify_payment(app: &AppHandle, t: &Transaction) {
    let amount = t.amount_paid.unwrap_or(t.amount);
    let body = format!("{} — {}", fmt_money(amount, &t.currency), kind_label(&t.kind));

    let _ = app
        .notification()
        .builder()
        .title("New payment 🎉")
        .body(body)
        .show();

    if let Some(state) = app.try_state::<AppState>() {
        state.play_sound();
    }
}

fn update_tray_totals(app: &AppHandle, paid: &[&Transaction]) {
    let start_of_today = Local
        .from_local_datetime(
            &Local::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .expect("valid midnight"),
        )
        .single()
        .map(|dt| dt.timestamp())
        .unwrap_or(0);

    let today: Vec<&&Transaction> = paid
        .iter()
        .filter(|t| t.created_at >= start_of_today)
        .collect();
    let count = today.len();
    let total: i64 = today
        .iter()
        .map(|t| t.amount_paid.unwrap_or(t.amount))
        .sum();
    let currency = today
        .first()
        .map(|t| t.currency.clone())
        .unwrap_or_default();

    let header = format!("Today: {} ({})", fmt_money(total, &currency), count);

    // Most recent payments (newest first) for the menu.
    let mut newest: Vec<&&Transaction> = paid.iter().collect();
    newest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let recent: Vec<String> = newest
        .iter()
        .take(tray::RECENT_SLOTS)
        .map(|t| fmt_money(t.amount_paid.unwrap_or(t.amount), &t.currency))
        .collect();

    tray::update_tray(app, header, recent);
}

fn fmt_money(cents: i64, currency: &str) -> String {
    let cur = if currency.is_empty() {
        String::new()
    } else {
        format!(" {}", currency.to_uppercase())
    };
    format!("{:.2}{}", cents as f64 / 100.0, cur)
}

fn kind_label(kind: &str) -> &str {
    match kind {
        "invoice" => "subscription",
        "payment" => "one-time",
        other if other.is_empty() => "payment",
        other => other,
    }
}
