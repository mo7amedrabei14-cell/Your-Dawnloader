"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

type Account = {
  id: string;
  email: string | null;
  email_verified: boolean;
  display_name: string | null;
  locale: string;
  preferences: { sync_download_history: boolean };
};
type DownloadRecord = {
  id: string;
  source_domain: string;
  provider_item_key: string | null;
  display_name: string | null;
  media_kind: string;
  container: string | null;
  quality_label: string | null;
  status: string;
  completed_at_unix_ms: number | null;
  created_at_unix_ms: number;
};
const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL?.replace(/\/$/, "");

async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  if (!API_BASE) throw new Error("Set NEXT_PUBLIC_API_BASE_URL to the public API origin.");
  const headers = new Headers(init?.headers);
  return fetch(API_BASE + path, { ...init, headers, credentials: "include" });
}
async function responseData<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { error?: string };
    throw new Error(body.error || "Request failed.");
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}
async function csrfToken(): Promise<string> {
  const result = await responseData<{ csrf_token: string }>(await apiFetch("/api/v1/csrf"));
  return result.csrf_token;
}

export default function MyDownloadsClient() {
  const [account, setAccount] = useState<Account | null>(null);
  const [records, setRecords] = useState<DownloadRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [filter, setFilter] = useState("all");

  const refresh = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const me = await apiFetch("/api/v1/me");
      if (me.status === 401) {
        setAccount(null);
        setRecords([]);
        return;
      }
      const user = await responseData<Account>(me);
      setAccount(user);
      if (!user.preferences.sync_download_history) {
        setRecords([]);
      } else {
        const page = await responseData<{ items: DownloadRecord[] }>(await apiFetch("/api/v1/downloads?limit=100"));
        setRecords(page.items);
      }
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Unable to connect to the account service.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  async function updateSync() {
    if (!account) return;
    setWorking(true); setError(""); setSuccess("");
    try {
      const token = await csrfToken();
      await responseData(await apiFetch("/api/v1/me/preferences", {
        method: "PUT",
        headers: { "Content-Type": "application/json", "X-CSRF-Token": token },
        body: JSON.stringify({ sync_download_history: !account.preferences.sync_download_history }),
      }));
      setSuccess(!account.preferences.sync_download_history
        ? "Download-history sync is enabled for this account."
        : "History sync is off. Existing synced records remain until removed.");
      await refresh();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not update privacy settings.");
    } finally { setWorking(false); }
  }

  async function removeRecord(id: string) {
    setWorking(true); setError(""); setSuccess("");
    try {
      const token = await csrfToken();
      await responseData(await apiFetch("/api/v1/downloads/" + encodeURIComponent(id), {
        method: "DELETE", headers: { "X-CSRF-Token": token },
      }));
      setRecords(rows => rows.filter(row => row.id !== id));
      setSuccess("Record removed. Any local media file remains untouched.");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not remove this record.");
    } finally { setWorking(false); }
  }

  async function clearHistory() {
    if (!window.confirm("Clear synced download history? Local media files will not be deleted.")) return;
    setWorking(true); setError(""); setSuccess("");
    try {
      const token = await csrfToken();
      const result = await responseData<{ deleted_records: number }>(await apiFetch("/api/v1/downloads", {
        method: "DELETE", headers: { "X-CSRF-Token": token },
      }));
      setRecords([]);
      setSuccess(result.deleted_records + " records removed from account history. Device files were not deleted.");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not clear account history.");
    } finally { setWorking(false); }
  }

  async function logout() {
    setWorking(true); setError(""); setSuccess("");
    try {
      const token = await csrfToken();
      await responseData(await apiFetch("/api/v1/auth/logout", {
        method: "POST", headers: { "X-CSRF-Token": token },
      }));
      setAccount(null); setRecords([]);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not sign out.");
    } finally { setWorking(false); }
  }

  const visibleRecords = records.filter(record => filter === "all" || record.media_kind === filter);

  return (
    <main className="site-shell">
      <nav className="nav">
        <Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link>
        <div className="nav-links"><Link href="/downloads">Downloads</Link><Link href="/security">Security</Link></div>
        {account
          ? <button className="nav-cta" onClick={logout} disabled={working}>Sign out</button>
          : <a className="nav-cta" href={API_BASE ? API_BASE + "/auth/oidc/login" : undefined}>Sign in ↗</a>}
      </nav>

      <section className="account-card account-library">
        <p className="eyebrow">PERSONAL LIBRARY</p>
        <h1>My Downloads</h1>
        <p className="muted">Your account is private. This page shows synced download records, not cloud copies of the files.</p>
        {!API_BASE && <div className="alert">Set NEXT_PUBLIC_API_BASE_URL to the public API origin before using sign-in.</div>}
        {error && <div className="alert" role="alert">{error}</div>}
        {success && <p className="success-message" role="status">{success}</p>}

        {loading ? <div className="empty-state"><p>Loading your account…</p></div> : !account ? (
          <div className="empty-state">
            <div className="symbol">◎</div>
            <h2>Sign in to your personal account</h2>
            <p>Each person has a separate account, private settings, and download history. Authentication is delegated to the configured identity provider.</p>
            {API_BASE && <a className="primary" href={API_BASE + "/auth/oidc/login"}>Continue to sign in ↗</a>}
          </div>
        ) : (
          <>
            <div className="account-summary">
              <div><span className="summary-label">SIGNED IN AS</span><strong>{account.display_name || account.email || "Your account"}</strong><span className="muted">{account.email || "Identity-provider account"}</span></div>
              <span className="account-private">Private account</span>
            </div>

            <div className="sync-setting">
              <div><strong>Sync download history</strong><p>Only metadata is synced. Songs, videos, and files remain on your devices by default.</p></div>
              <button className={account.preferences.sync_download_history ? "toggle enabled" : "toggle"} role="switch" aria-checked={account.preferences.sync_download_history} onClick={updateSync} disabled={working}><span /></button>
            </div>

            {!account.preferences.sync_download_history ? (
              <div className="empty-state"><div className="symbol">↧</div><h2>History sync is off</h2><p>Enable this setting to view and sync download records. You can keep all history local.</p></div>
            ) : (
              <>
                <div className="library-toolbar">
                  <div><h2>Download history</h2><span>{records.length} records loaded</span></div>
                  <div className="toolbar-actions">
                    <select aria-label="Filter media type" value={filter} onChange={event => setFilter(event.target.value)}>
                      <option value="all">All media</option><option value="audio">Audio</option><option value="video">Video</option><option value="image">Images</option><option value="document">Documents</option>
                    </select>
                    <button className="secondary" onClick={clearHistory} disabled={working || records.length === 0}>Clear history</button>
                  </div>
                </div>
                {visibleRecords.length === 0 ? (
                  <div className="empty-state"><div className="symbol">↧</div><h2>No synced downloads yet</h2><p>Records synced by your signed-in clients appear here; the media file itself is not uploaded.</p></div>
                ) : (
                  <div className="account-download-list">
                    {visibleRecords.map(record => (
                      <article className="account-download-row" key={record.id}>
                        <span className="file-mark">{record.media_kind === "audio" ? "♫" : record.media_kind === "video" ? "▷" : "↧"}</span>
                        <div className="download-record-info">
                          <strong>{record.display_name || record.provider_item_key || "Download"}</strong>
                          <span>{record.source_domain} · {(record.container || record.media_kind).toUpperCase()}{record.quality_label ? " · " + record.quality_label : ""}</span>
                          <small>{record.status} · {new Date(record.completed_at_unix_ms ?? record.created_at_unix_ms).toLocaleString()}</small>
                        </div>
                        <button className="text-danger" onClick={() => void removeRecord(record.id)} disabled={working}>Remove</button>
                      </article>
                    ))}
                  </div>
                )}
              </>
            )}
            <p className="privacy-note">Removing an account-history record does not delete the original file from any device.</p>
          </>
        )}
      </section>
      <footer className="site-footer"><span>YOUR DAWNLOADER · ACCOUNT & LIBRARY</span><span><Link href="/privacy">Privacy</Link> · <Link href="/security">Security</Link></span></footer>
    </main>
  );
}
