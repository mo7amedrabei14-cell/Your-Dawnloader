import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

type Inspection = {
  host: string;
  source_kind: string;
  media_kind: string;
  note: string;
};

type DownloadStatus = "queued" | "downloading" | "completed" | "failed" | "cancelled";
type DownloadProgress = {
  task_id: string;
  status: DownloadStatus;
  file_name: string | null;
  downloaded_bytes: number;
  total_bytes: number | null;
  message: string | null;
};
type ActiveDownload = {
  taskId: string;
  status: DownloadStatus;
  fileName: string;
  downloadedBytes: number;
  totalBytes: number | null;
  message: string | null;
};
type LocalDownloadRecord = {
  id: string;
  title: string;
  source_domain: string;
  media_kind: string;
  format: string | null;
  file_name: string;
  size_bytes: number;
  completed_at_unix_ms: number;
  file_exists: boolean;
};

type LinkedAccount = {
  connected: boolean;
  display_name: string | null;
  email: string | null;
  sync_download_history: boolean;
};
type DeviceLinkStart = {
  link_id: string;
  user_code: string;
  verification_url: string;
  expires_in_seconds: number;
  interval_seconds: number;
};
type DeviceLinkStatus = {
  status: "pending" | "authorized" | "expired";
  display_name: string | null;
  email: string | null;
  sync_download_history: boolean | null;
  device_label: string | null;
};
type PairingState = DeviceLinkStart;

function formatBytes(value: number): string {
  if (value < 1024) return value + " B";
  const units = ["KB", "MB", "GB", "TB"];
  let size = value / 1024;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return size.toFixed(1) + " " + units[index];
}

function statusLabel(status: DownloadStatus): string {
  return ({
    queued: "In queue",
    downloading: "Downloading",
    completed: "Completed",
    failed: "Failed",
    cancelled: "Cancelled",
  })[status];
}

export default function App() {
  const [url, setUrl] = useState("");
  const [result, setResult] = useState<Inspection | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [activeView, setActiveView] = useState<"new" | "downloads">("new");
  const [tasks, setTasks] = useState<ActiveDownload[]>([]);
  const [records, setRecords] = useState<LocalDownloadRecord[]>([]);
  const [historyError, setHistoryError] = useState("");
  const [account, setAccount] = useState<LinkedAccount | null>(null);
  const [accountError, setAccountError] = useState("");
  const [accountBusy, setAccountBusy] = useState(false);
  const [pairing, setPairing] = useState<PairingState | null>(null);

  async function loadHistory() {
    try {
      setHistoryError("");
      setRecords(await invoke<LocalDownloadRecord[]>("list_downloads"));
    } catch (caught) {
      setHistoryError(typeof caught === "string" ? caught : "Could not load local download history.");
    }
  }

  async function loadAccount() {
    try {
      setAccountError("");
      setAccount(await invoke<LinkedAccount | null>("get_account_status"));
    } catch (caught) {
      setAccountError(typeof caught === "string" ? caught : "Could not check account connection.");
    }
  }

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    listen<DownloadProgress>("download-progress", (event) => {
      const update = event.payload;
      setTasks((current) => {
        const existing = current.find((task) => task.taskId === update.task_id);
        const next: ActiveDownload = {
          taskId: update.task_id,
          status: update.status,
          fileName: update.file_name ?? existing?.fileName ?? "Preparing download",
          downloadedBytes: update.downloaded_bytes,
          totalBytes: update.total_bytes ?? existing?.totalBytes ?? null,
          message: update.message,
        };
        return [next, ...current.filter((task) => task.taskId !== update.task_id)].slice(0, 30);
      });
      if (update.status === "completed") {
        void invoke<LocalDownloadRecord[]>("list_downloads")
          .then(setRecords)
          .catch(() => setHistoryError("The file downloaded, but its local history could not be refreshed."));
        setActiveView("downloads");
      }
    }).then((handler) => {
      if (active) unlisten = handler;
      else handler();
    });

    void loadHistory();
    void loadAccount();
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  async function connectAccount() {
    setAccountBusy(true); setAccountError("");
    try {
      const link = await invoke<DeviceLinkStart>("start_account_link");
      setPairing(link);
      try {
        await openUrl(link.verification_url);
      } catch {
        setAccountError("The approval page could not be opened automatically. Use the URL shown below.");
      }
    } catch (caught) {
      setAccountError(typeof caught === "string" ? caught : "Could not start account connection.");
    } finally {
      setAccountBusy(false);
    }
  }

  async function openApprovalPage() {
    if (!pairing) return;
    try {
      await openUrl(pairing.verification_url);
    } catch {
      setAccountError("Could not open the verification URL. Copy it into your browser manually.");
    }
  }

  async function toggleAccountSync() {
    if (!account) return;
    setAccountBusy(true); setAccountError("");
    try {
      const updated = await invoke<LinkedAccount>("set_account_sync", {
        enabled: !account.sync_download_history,
      });
      setAccount(updated);
    } catch (caught) {
      setAccountError(typeof caught === "string" ? caught : "Could not update cloud history sync.");
      await loadAccount();
    } finally {
      setAccountBusy(false);
    }
  }

  async function disconnectAccount() {
    setAccountBusy(true); setAccountError("");
    try {
      await invoke<boolean>("disconnect_account");
      setAccount(null);
    } catch (caught) {
      setAccountError(typeof caught === "string" ? caught : "Could not disconnect the account.");
    } finally {
      setAccountBusy(false);
    }
  }

  useEffect(() => {
    if (!pairing) return;
    let cancelled = false;
    let timeout: ReturnType<typeof setTimeout> | undefined;

    const poll = async () => {
      try {
        const status = await invoke<DeviceLinkStatus>("poll_account_link", { linkId: pairing.link_id });
        if (cancelled) return;
        if (status.status === "authorized") {
          setAccount({
            connected: true,
            display_name: status.display_name,
            email: status.email,
            sync_download_history: status.sync_download_history ?? false,
          });
          setPairing(null);
          setAccountError("");
          return;
        }
        if (status.status === "expired") {
          setPairing(null);
          setAccountError("This connection code expired. Start a new connection and approve it within 10 minutes.");
          return;
        }
        timeout = setTimeout(poll, pairing.interval_seconds * 1000);
      } catch (caught) {
        if (cancelled) return;
        setAccountError(typeof caught === "string" ? caught : "Could not check approval. The app will keep retrying.");
        timeout = setTimeout(poll, pairing.interval_seconds * 1000);
      }
    };

    timeout = setTimeout(poll, pairing.interval_seconds * 1000);
    return () => {
      cancelled = true;
      if (timeout) clearTimeout(timeout);
      void invoke<boolean>("cancel_account_link", { linkId: pairing.link_id }).catch(() => undefined);
    };
  }, [pairing?.link_id]);

  async function inspect() {
    setError("");
    setResult(null);
    setBusy(true);
    try {
      const value = await invoke<Inspection>("inspect_url", { url });
      setResult(value);
    } catch (caught) {
      setError(typeof caught === "string" ? caught : "The URL could not be inspected.");
    } finally {
      setBusy(false);
    }
  }

  async function startDownload() {
    setError("");
    try {
      await invoke<{ task_id: string }>("start_download", { url });
      setError("");
    } catch (caught) {
      setError(typeof caught === "string" ? caught : "Could not start this download.");
    }
  }

  async function cancelDownload(taskId: string) {
    try {
      await invoke<boolean>("cancel_download", { taskId });
    } catch {
      setError("Could not cancel the download.");
    }
  }

  async function deleteRecord(id: string) {
    try {
      await invoke<boolean>("delete_download_record", { id });
      await loadHistory();
    } catch {
      setHistoryError("Could not remove this item from history.");
    }
  }

  async function clearHistory() {
    const approved = window.confirm("Clear download history? Files already saved on disk will not be deleted.");
    if (!approved) return;
    try {
      await invoke<number>("clear_download_history");
      await loadHistory();
    } catch {
      setHistoryError("Could not clear download history.");
    }
  }

  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">D</span><span>Your Dawnloader</span></div>
        <div className="topbar-actions">
          <nav className="view-switch" aria-label="Application views">
            <button className={activeView === "new" ? "selected" : ""} onClick={() => setActiveView("new")}>New</button>
            <button className={activeView === "downloads" ? "selected" : ""} onClick={() => { setActiveView("downloads"); void loadHistory(); }}>My Downloads</button>
          </nav>
          {account ? (
            <div className="account-tools">
              <div className="account-chip"><span className="status-dot" /><span>{account.display_name || account.email || "Connected account"}</span></div>
              <button className={account.sync_download_history ? "sync-pill enabled" : "sync-pill"} onClick={() => void toggleAccountSync()} disabled={accountBusy} title="Toggle cloud download-history sync">
                Sync {account.sync_download_history ? "on" : "off"}
              </button>
              <button className="text-button" onClick={() => void disconnectAccount()} disabled={accountBusy}>Disconnect</button>
            </div>
          ) : (
            <button className="connect-button" onClick={() => void connectAccount()} disabled={accountBusy || !!pairing}>
              {accountBusy ? "Connecting…" : "Connect account ↗"}
            </button>
          )}
        </div>
      </header>

      {accountError && <p className="notice error account-notice" role="alert">{accountError}</p>}
      {pairing && (
        <section className="pairing-panel" aria-live="polite">
          <div className="pairing-copy">
            <p className="eyebrow">APPROVE THIS DEVICE</p>
            <h2>Finish connecting your account</h2>
            <p>Sign in on the page that opened and approve only if this code matches the one shown here.</p>
            <div className="pairing-code">{pairing.user_code}</div>
            <small>Expires in about {Math.max(1, Math.ceil(pairing.expires_in_seconds / 60))} minutes · checking every {pairing.interval_seconds} seconds</small>
            <div className="pairing-actions">
              <button className="download-now" onClick={() => void openApprovalPage()}>Open approval page ↗</button>
              <button className="text-button" onClick={() => setPairing(null)}>Cancel</button>
            </div>
            <a className="pairing-url" href={pairing.verification_url} onClick={(event) => { event.preventDefault(); void openApprovalPage(); }}>{pairing.verification_url}</a>
          </div>
        </section>
      )}

      {activeView === "new" ? (
        <>
          <section className="hero">
            <p className="eyebrow">DISCOVER · ORGANIZE · DOWNLOAD</p>
            <h1>Your media.<br /><span>Under your control.</span></h1>
            <p className="hero-copy">Analyze a media URL, check the source type, and save direct files to your device. Protected streams are not bypassed.</p>

            <section className="url-card" aria-labelledby="url-heading">
              <label id="url-heading" htmlFor="media-url">Paste a media link</label>
              <div className="url-row">
                <input id="media-url" type="url" value={url} onChange={(event) => { setUrl(event.target.value); setResult(null); }} placeholder="https://example.com/video.mp4" autoComplete="url" />
                <button onClick={inspect} disabled={busy || !url.trim()}>{busy ? "Checking…" : "Analyze link"} <span aria-hidden="true">↗</span></button>
              </div>
              <p className="field-note">URL inspection is local. This build does not send the pasted link to an account service.</p>
              {error && <p className="notice error" role="alert">{error}</p>}
              {result && (
                <div className="result-panel" role="status">
                  <div><span className="result-label">SOURCE</span><strong>{result.host}</strong></div>
                  <div><span className="result-label">TYPE</span><strong>{result.media_kind}</strong></div>
                  <div><span className="result-label">DETECTION</span><strong>{result.source_kind.replaceAll("_", " ")}</strong></div>
                  <p>{result.note}</p>
                  {result.source_kind === "direct_file" && (
                    <button className="download-now" onClick={startDownload}>Download direct file <span aria-hidden="true">↓</span></button>
                  )}
                </div>
              )}
            </section>
          </section>

          <section className="task-section">
            <div className="section-heading"><h2>Download activity</h2><span>{tasks.filter((task) => task.status === "queued" || task.status === "downloading").length} active</span></div>
            {tasks.length === 0 ? <p className="empty-note">Your active downloads and recent outcomes will appear here.</p> : (
              <div className="task-list">
                {tasks.map((task) => {
                  const percent = task.totalBytes && task.totalBytes > 0
                    ? Math.min(100, (task.downloadedBytes / task.totalBytes) * 100)
                    : null;
                  const canCancel = task.status === "queued" || task.status === "downloading";
                  return (
                    <article className="task-row" key={task.taskId}>
                      <span className={"task-symbol " + task.status}>{task.status === "completed" ? "✓" : task.status === "failed" ? "!" : "↓"}</span>
                      <div className="task-main">
                        <strong>{task.fileName}</strong>
                        <span>{statusLabel(task.status)} · {formatBytes(task.downloadedBytes)}{task.totalBytes ? " / " + formatBytes(task.totalBytes) : ""}</span>
                        {percent !== null && <div className="progress-track"><div className="progress-value" style={{ width: percent + "%" }} /></div>}
                        {task.message && <small>{task.message}</small>}
                      </div>
                      {canCancel && <button className="text-button" onClick={() => void cancelDownload(task.taskId)}>Cancel</button>}
                    </article>
                  );
                })}
              </div>
            )}
          </section>
        </>
      ) : (
        <section className="library">
          <div className="library-heading">
            <div><p className="eyebrow">LOCAL LIBRARY</p><h1>My Downloads</h1><p className="hero-copy">Files downloaded on this device. Removing a history record does not delete its file.</p></div>
            <button className="text-button" onClick={clearHistory} disabled={records.length === 0}>Clear history</button>
          </div>
          {historyError && <p className="notice error" role="alert">{historyError}</p>}
          {records.length === 0 ? (
            <div className="empty-library"><span>↓</span><h2>No completed downloads yet</h2><p>When a direct file finishes downloading, it will appear here with its local availability.</p></div>
          ) : (
            <div className="library-list">
              {records.map((record) => (
                <article className="library-row" key={record.id}>
                  <span className="file-mark">{record.media_kind === "audio" ? "♫" : record.media_kind === "video" ? "▷" : "↧"}</span>
                  <div className="library-main">
                    <strong>{record.title}</strong>
                    <span>{record.source_domain} · {record.format?.toUpperCase() || "FILE"} · {formatBytes(record.size_bytes)}</span>
                    <small>{new Date(record.completed_at_unix_ms).toLocaleString()} · {record.file_exists ? "Present on this device" : "File not found at its saved location"}</small>
                  </div>
                  <button className="text-button" onClick={() => void deleteRecord(record.id)}>Remove record</button>
                </article>
              ))}
            </div>
          )}
          <p className="privacy-note">This is the history saved on this device. Connect an account and enable sync to send metadata to My Downloads on the website; the media files stay local.</p>
        </section>
      )}

      <section className="feature-grid" aria-label="Product areas">
        <article className="feature-card"><span className="feature-icon">↧</span><div><h2>Direct downloads</h2><p>Bounded streaming, progress, cancellation, and safe local history for direct files.</p><span className="state-label">EARLY ENGINE</span></div></article>
        <article className="feature-card"><span className="feature-icon">◉</span><div><h2>Browser capture</h2><p>Find eligible media candidates with an explicit user action and minimal permissions.</p><span className="state-label">EXTENSION FOUNDATION</span></div></article>
        <article className="feature-card"><span className="feature-icon">◎</span><div><h2>Account library</h2><p>Connect this device and opt in to sync download metadata to your website account.</p><span className="state-label">{account ? (account.sync_download_history ? "ACCOUNT CONNECTED · SYNC ON" : "ACCOUNT CONNECTED · LOCAL ONLY") : "OPTIONAL DEVICE LINK"}</span></div></article>
      </section>
      <footer><span>LOCAL-FIRST BY DESIGN</span><span>Direct media detection does not imply download permission.</span></footer>
    </main>
  );
}
