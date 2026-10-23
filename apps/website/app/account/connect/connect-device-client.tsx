"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

type Account = { display_name: string | null; email: string | null };
const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL?.replace(/\/$/, "");

async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  if (!API_BASE) throw new Error("Set NEXT_PUBLIC_API_BASE_URL to the public API origin.");
  return fetch(API_BASE + path, { ...init, credentials: "include", headers: new Headers(init?.headers) });
}
async function parseData<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { error?: string };
    throw new Error(body.error || "Request failed.");
  }
  return response.json() as Promise<T>;
}
async function csrfToken(): Promise<string> {
  return (await parseData<{ csrf_token: string }>(await apiFetch("/api/v1/csrf"))).csrf_token;
}

export default function ConnectDeviceClient({ initialCode }: { initialCode: string }) {
  const [code, setCode] = useState(initialCode.toUpperCase());
  const [account, setAccount] = useState<Account | null>(null);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [approved, setApproved] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true); setError("");
    try {
      const response = await apiFetch("/api/v1/me");
      if (response.status === 401) { setAccount(null); return; }
      const data = await parseData<Account>(response);
      setAccount(data);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not contact the account service.");
    } finally { setLoading(false); }
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);

  async function approveDevice() {
    const normalized = code.toUpperCase().trim();
    if (!/^[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{4}-?[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{4}$/.test(normalized)) {
      setError("Enter the 8-character code shown in your desktop app.");
      return;
    }
    setWorking(true); setError(""); setSuccess("");
    try {
      const token = await csrfToken();
      await parseData(await apiFetch("/api/v1/device-auth/approve", {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-CSRF-Token": token },
        body: JSON.stringify({ user_code: normalized }),
      }));
      setApproved(true);
      setSuccess("Device connected. Return to Your Dawnloader on your computer.");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not approve this device.");
    } finally { setWorking(false); }
  }

  const signInUrl = API_BASE + "/auth/oidc/login?return_to=" + encodeURIComponent("/account/connect?user_code=" + code.trim().toUpperCase());

  return (
    <main className="site-shell">
      <nav className="nav">
        <Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link>
        <Link className="nav-cta" href="/account/downloads">My Downloads</Link>
      </nav>
      <section className="account-card account-library">
        <p className="eyebrow">SECURE DEVICE LINK</p>
        <h1>Connect your desktop</h1>
        <p className="muted">Approve a device you control to sync download-history metadata. This does not upload or copy your media files.</p>
        {!API_BASE && <div className="alert">Set NEXT_PUBLIC_API_BASE_URL to the public API origin before connecting a device.</div>}
        {error && <div className="alert" role="alert">{error}</div>}
        {success && <p className="success-message" role="status">{success}</p>}
        {loading ? <div className="empty-state"><p>Checking your account…</p></div> : !account ? (
          <div className="empty-state">
            <div className="symbol">◎</div>
            <h2>Sign in before approving</h2>
            <p>Only the account you sign in to receives this device’s download metadata.</p>
            {API_BASE && <a className="primary" href={signInUrl}>Sign in to continue ↗</a>}
          </div>
        ) : approved ? (
          <div className="empty-state"><div className="symbol">✓</div><h2>Device approved</h2><p>The desktop app should finish connecting shortly. You can close this page.</p></div>
        ) : (
          <>
            <div className="account-summary">
              <div><span className="summary-label">CONNECTING TO ACCOUNT</span><strong>{account.display_name || account.email || "Your account"}</strong><span className="muted">{account.email || "Signed-in account"}</span></div>
              <span className="account-private">Private account</span>
            </div>
            <div className="connect-code-form">
              <label htmlFor="device-code">Desktop connection code</label>
              <input id="device-code" autoComplete="off" maxLength={9} value={code} onChange={event => setCode(event.target.value.toUpperCase())} placeholder="ABCD-EFGH" />
              <p className="muted">Only approve this code if it is currently visible in the desktop app you launched.</p>
              <button className="primary" onClick={() => void approveDevice()} disabled={working || !code.trim()}>Approve this device</button>
            </div>
          </>
        )}
      </section>
      <footer className="site-footer"><span>YOUR DAWNLOADER · DEVICE LINK</span><span><Link href="/privacy">Privacy</Link> · <Link href="/security">Security</Link></span></footer>
    </main>
  );
}
