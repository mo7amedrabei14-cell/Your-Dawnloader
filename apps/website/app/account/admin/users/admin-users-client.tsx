"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

type Role = "owner" | "admin" | "vip_user" | "regular_user";
type Status = "active" | "suspended";
type AdminUser = {
  id: string;
  role: Role;
  account_status: Status;
  email: string | null;
  email_verified: boolean;
  display_name: string | null;
  locale: string | null;
  created_at: string | null;
};
type Edit = { role: Exclude<Role, "owner">; account_status: Status; display_name: string; locale: "en" | "ar" };
const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL?.replace(/\/$/, "");

async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  if (!API_BASE) throw new Error("Set NEXT_PUBLIC_API_BASE_URL to the public API origin.");
  return fetch(API_BASE + path, { ...init, credentials: "include", headers: new Headers(init?.headers) });
}
async function responseData<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { error?: string };
    throw new Error(body.error || "Request failed.");
  }
  return response.json() as Promise<T>;
}
async function csrfToken(): Promise<string> {
  return (await responseData<{ csrf_token: string }>(await apiFetch("/api/v1/csrf"))).csrf_token;
}
function initialEdit(user: AdminUser): Edit {
  return {
    role: user.role === "owner" ? "regular_user" : user.role,
    account_status: user.account_status,
    display_name: user.display_name || "",
    locale: user.locale === "ar" ? "ar" : "en",
  };
}
const roleLabels: Record<Role, string> = {
  owner: "Owner",
  admin: "Admin (reserved)",
  vip_user: "VIP User",
  regular_user: "Regular User",
};

export default function AdminUsersClient() {
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [edits, setEdits] = useState<Record<string, Edit>>({});
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const load = useCallback(async () => {
    setLoading(true); setError("");
    try {
      const data = await responseData<{ users: AdminUser[] }>(await apiFetch("/api/v1/admin/users?limit=100"));
      setUsers(data.users);
      setEdits(Object.fromEntries(data.users.map(user => [user.id, initialEdit(user)])));
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not load the user list.");
    } finally { setLoading(false); }
  }, []);
  useEffect(() => { void load(); }, [load]);

  function updateEdit(id: string, update: Partial<Edit>) {
    setEdits(previous => ({ ...previous, [id]: { ...previous[id], ...update } }));
  }

  async function save(user: AdminUser) {
    const edit = edits[user.id];
    if (!edit || user.role === "owner") return;
    setWorking(user.id); setError(""); setNotice("");
    try {
      const token = await csrfToken();
      const updated = await responseData<{ user: AdminUser }>(await apiFetch("/api/v1/admin/users/" + encodeURIComponent(user.id), {
        method: "PATCH",
        headers: { "Content-Type": "application/json", "X-CSRF-Token": token },
        body: JSON.stringify({
          role: edit.role,
          account_status: edit.account_status,
          display_name: edit.display_name.trim() || undefined,
          locale: edit.locale,
        }),
      }));
      setNotice("User updated: " + (updated.user.email || updated.user.id));
      await load();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Could not save this user.");
    } finally { setWorking(null); }
  }

  return (
    <main className="site-shell">
      <nav className="nav">
        <Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link>
        <div className="nav-links"><Link href="/account/downloads">My Downloads</Link><Link href="/security">Security</Link></div>
        <Link className="nav-cta" href="/account/downloads">Account ↗</Link>
      </nav>
      <section className="account-card admin-card">
        <p className="eyebrow">OWNER CONSOLE</p>
        <h1>User administration</h1>
        <p className="muted">Only the verified Owner session can use this screen. New accounts appear after their first successful sign-in. The Owner account is protected from editing here.</p>
        {!API_BASE && <div className="alert">Set NEXT_PUBLIC_API_BASE_URL in GitHub Actions before using this page.</div>}
        {error && <div className="alert" role="alert">{error}</div>}
        {notice && <p className="success-message" role="status">{notice}</p>}
        {loading ? <div className="empty-state"><p>Loading accounts…</p></div> : users.length === 0 ? (
          <div className="empty-state"><h2>No users available</h2><p>Sign in with the configured Owner email first, then return here.</p></div>
        ) : (
          <div className="admin-user-list">
            {users.map(user => {
              const edit = edits[user.id] || initialEdit(user);
              const locked = user.role === "owner";
              return (
                <article className="admin-user" key={user.id}>
                  <div className="admin-user-heading">
                    <div><strong>{user.display_name || user.email || "Account"}</strong><span>{user.email || "No verified email"}{user.email_verified ? " · verified" : " · unverified"}</span></div>
                    <span className={user.account_status === "active" ? "admin-status active" : "admin-status suspended"}>{user.account_status}</span>
                  </div>
                  <div className="admin-user-fields">
                    <label>Role
                      <select value={locked ? "regular_user" : edit.role} disabled={locked || working === user.id} onChange={event => updateEdit(user.id, { role: event.target.value as Edit["role"] })}>
                        <option value="regular_user">Regular User</option>
                        <option value="vip_user">VIP User</option>
                        <option value="admin">Admin (reserved)</option>
                      </select>
                    </label>
                    <label>Account status
                      <select value={edit.account_status} disabled={locked || working === user.id} onChange={event => updateEdit(user.id, { account_status: event.target.value as Status })}>
                        <option value="active">Active</option>
                        <option value="suspended">Suspended</option>
                      </select>
                    </label>
                    <label>Display name
                      <input value={edit.display_name} disabled={locked || working === user.id} maxLength={80} onChange={event => updateEdit(user.id, { display_name: event.target.value })} />
                    </label>
                    <label>Language
                      <select value={edit.locale} disabled={locked || working === user.id} onChange={event => updateEdit(user.id, { locale: event.target.value as Edit["locale"] })}>
                        <option value="en">English</option><option value="ar">Arabic</option>
                      </select>
                    </label>
                  </div>
                  <div className="admin-user-footer">
                    <small>Created {user.created_at ? new Date(user.created_at).toLocaleDateString() : "date unavailable"} · ID {user.id}</small>
                    {locked ? <span className="account-private">Owner — protected</span> : <button className="primary" onClick={() => void save(user)} disabled={working === user.id}>{working === user.id ? "Saving…" : "Save changes"}</button>}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>
      <footer className="site-footer"><span>YOUR DAWNLOADER · OWNER CONSOLE</span><span><Link href="/account/downloads">My Downloads</Link></span></footer>
    </main>
  );
}
