# creembar

A tiny cross-platform **system-tray app** that notifies you — with a sound — whenever a new payment lands in your [creem.io](https://creem.io) account. Inspired by [datafastbar](https://www.joshmillgate.co.uk/projects/datafastbar).

## How it works

creembar lives in your tray/menu bar. It stores your creem **API key** in the OS keychain and polls `GET /v1/transactions/search` on an interval (default 60s). When a new transaction with status `paid` appears, it shows a native notification and plays a "cha-ching" sound. The tray menu shows today's total and your latest payments.

There is **no server and no webhook** — a desktop app has no public URL, so creembar pulls directly from the creem API.

> The API key is a full-access secret. It is kept in the OS keychain (`keyring`), never in a plaintext file, and is never exposed to the web frontend.

## Setup

1. Launch creembar — it appears in the tray (no dock icon on macOS).
2. Open **Settings…** from the tray menu.
3. Paste your creem API key, choose **Test mode** (uses `test-api.creem.io`) or production, set the poll interval, optionally enable **Start at login**.
4. Click **Test connection**, then **Save**.

The first successful poll records existing transactions silently (no notification spam); only payments that arrive afterwards trigger a notification + sound.

## Develop

```bash
bun install
bun run tauri dev
```

## Build / Release

Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds **unsigned** artifacts for macOS / Windows / Linux and attaches them to a draft GitHub Release.

Because the builds are unsigned:
- **macOS:** right-click the app → **Open**, or run `xattr -dr com.apple.quarantine creembar.app`.
- **Windows:** SmartScreen → **More info → Run anyway**.

## Tech

Tauri v2 (Rust) · React 19 + Vite · Bun. Backend modules: `creem.rs` (API client), `poller.rs` (polling + dedup), `tray.rs`, `keychain.rs`, `sound.rs`, `commands.rs`.
