import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Settings = {
  test_mode: boolean;
  interval_secs: number;
  autostart: boolean;
  has_api_key: boolean;
};

type Status = { kind: "idle" | "ok" | "error" | "busy"; message: string };

function App() {
  const [testMode, setTestMode] = useState(true);
  const [intervalSecs, setIntervalSecs] = useState(60);
  const [autostart, setAutostart] = useState(false);
  const [hasApiKey, setHasApiKey] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState<Status>({ kind: "idle", message: "" });

  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => {
        setTestMode(s.test_mode);
        setIntervalSecs(s.interval_secs);
        setAutostart(s.autostart);
        setHasApiKey(s.has_api_key);
      })
      .catch((e) => setStatus({ kind: "error", message: String(e) }));
  }, []);

  async function save() {
    setStatus({ kind: "busy", message: "Saving…" });
    try {
      const interval = Math.max(15, Math.floor(intervalSecs) || 60);
      await invoke("save_settings", {
        testMode,
        intervalSecs: interval,
        autostart,
      });
      if (apiKey.trim()) {
        await invoke("set_api_key", { apiKey: apiKey.trim() });
        setApiKey("");
        setHasApiKey(true);
      }
      setStatus({ kind: "ok", message: "Saved." });
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  }

  async function testConnection() {
    setStatus({ kind: "busy", message: "Testing…" });
    try {
      // Test the key + mode exactly as shown, without saving first.
      const count = await invoke<number>("test_connection", {
        apiKey: apiKey.trim() || null,
        testMode,
      });
      setStatus({
        kind: "ok",
        message: `Connected (${testMode ? "test" : "production"}) — ${count} transaction(s) visible.`,
      });
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  }

  async function playTestSound() {
    try {
      await invoke("play_test_sound");
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  }

  async function clearKey() {
    try {
      await invoke("clear_api_key");
      setHasApiKey(false);
      setApiKey("");
      setStatus({ kind: "ok", message: "API key removed." });
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  }

  return (
    <main className="app">
      <header>
        <h1>creembar</h1>
        <p className="sub">Payment notifications in your tray</p>
      </header>

      <label className="field">
        <span>creem API key</span>
        <input
          type="password"
          autoComplete="off"
          placeholder={hasApiKey ? "•••••••• (saved — type to replace)" : "creem_…"}
          value={apiKey}
          onChange={(e) => setApiKey(e.currentTarget.value)}
        />
        {hasApiKey && (
          <button className="link" type="button" onClick={clearKey}>
            Remove saved key
          </button>
        )}
      </label>

      <label className="field row">
        <span>Test mode</span>
        <input
          type="checkbox"
          checked={testMode}
          onChange={(e) => setTestMode(e.currentTarget.checked)}
        />
      </label>

      <label className="field">
        <span>Poll interval (seconds)</span>
        <input
          type="number"
          min={15}
          value={intervalSecs}
          onChange={(e) => setIntervalSecs(Number(e.currentTarget.value))}
        />
      </label>

      <label className="field row">
        <span>Start at login</span>
        <input
          type="checkbox"
          checked={autostart}
          onChange={(e) => setAutostart(e.currentTarget.checked)}
        />
      </label>

      <div className="field row">
        <span>Notification sound</span>
        <button type="button" className="secondary" onClick={playTestSound}>
          ▶ Play
        </button>
      </div>

      <div className="actions">
        <button type="button" className="secondary" onClick={testConnection}>
          Test connection
        </button>
        <button type="button" onClick={save}>
          Save
        </button>
      </div>

      {status.message && (
        <p className={`status ${status.kind}`}>{status.message}</p>
      )}
    </main>
  );
}

export default App;
