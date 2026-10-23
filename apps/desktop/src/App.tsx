import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Inspection = {
  host: string;
  source_kind: string;
  media_kind: string;
  note: string;
};

export default function App() {
  const [url, setUrl] = useState("");
  const [result, setResult] = useState<Inspection | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

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

  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">D</span><span>Your Dawnloader</span></div>
        <div className="account-chip"><span className="status-dot" /> Local mode</div>
      </header>

      <section className="hero">
        <p className="eyebrow">DISCOVER · ORGANIZE · DOWNLOAD</p>
        <h1>Your media.<br /><span>Under your control.</span></h1>
        <p className="hero-copy">One thoughtful workspace for media you are allowed to save, from direct links to browser discoveries.</p>

        <section className="url-card" aria-labelledby="url-heading">
          <label id="url-heading" htmlFor="media-url">Paste a media link</label>
          <div className="url-row">
            <input id="media-url" type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://example.com/video.mp4" autoComplete="url" />
            <button onClick={inspect} disabled={busy || !url.trim()}>{busy ? "Checking…" : "Analyze link"} <span aria-hidden="true">↗</span></button>
          </div>
          <p className="field-note">URL inspection is local. We do not send this link to an account service.</p>
          {error && <p className="notice error" role="alert">{error}</p>}
          {result && (
            <div className="result-panel" role="status">
              <div><span className="result-label">SOURCE</span><strong>{result.host}</strong></div>
              <div><span className="result-label">TYPE</span><strong>{result.media_kind}</strong></div>
              <div><span className="result-label">DETECTION</span><strong>{result.source_kind}</strong></div>
              <p>{result.note}</p>
            </div>
          )}
        </section>
      </section>

      <section className="feature-grid" aria-label="Product areas">
        <article className="feature-card"><span className="feature-icon">↧</span><div><h2>Downloads</h2><p>Queue, progress, retry and resume when the source supports it.</p><span className="state-label">Engine foundation started</span></div></article>
        <article className="feature-card"><span className="feature-icon">◉</span><div><h2>Browser capture</h2><p>Find media candidates from a page without granting silent access to every site.</p><span className="state-label">Extension scaffold planned</span></div></article>
        <article className="feature-card"><span className="feature-icon">◎</span><div><h2>My Downloads</h2><p>Your account library will sync download metadata only when you opt in.</p><span className="state-label">Account integration pending</span></div></article>
      </section>
      <footer><span>LOCAL-FIRST BY DESIGN</span><span>Direct media detection does not imply download permission.</span></footer>
    </main>
  );
}
