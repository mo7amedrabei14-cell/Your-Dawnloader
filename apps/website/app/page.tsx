import Link from "next/link";

const features = [
  { icon: "↧", title: "A considered download engine", copy: "Queues, progress, retries, safe file handling, and resume where the source supports it.", state: "ENGINE IN DEVELOPMENT" },
  { icon: "◉", title: "Media discovery in your browser", copy: "Find eligible media candidates with explicit permissions and clear source information.", state: "EXTENSION FOUNDATION" },
  { icon: "◎", title: "Your personal library", copy: "A private account library for download history, with optional metadata sync across devices.", state: "ACCOUNT SERVICE IN DEVELOPMENT" },
];

export default function HomePage() {
  return (
    <main className="site-shell">
      <nav className="nav" aria-label="Main navigation">
        <Link className="brand" href="/"><span className="brand-mark">D</span><span>Your Dawnloader</span></Link>
        <div className="nav-links"><Link href="#features">Features</Link><Link href="/downloads">Downloads</Link><Link href="/account/downloads">My Downloads</Link><Link href="/security">Security</Link><Link href="/login">Sign in</Link></div>
        <Link className="nav-cta" href="/login">Sign in / Create account ↗</Link>
      </nav>

      <section className="hero" data-reveal>
        <p className="eyebrow">DISCOVER · ORGANIZE · DOWNLOAD</p>
        <h1>Media, without<br /><span>the mess.</span></h1>
        <p className="lead">One thoughtful workspace for discovering eligible media, managing downloads, and keeping your library under your control — across desktop and browser.</p>
        <div className="hero-actions"><Link className="primary" href="/downloads">Explore platforms ↗</Link><Link className="secondary" href="#features">See how it works</Link></div>
        <div className="pill-row"><span className="pill">Windows</span><span className="pill">macOS</span><span className="pill">Linux</span><span className="pill">Browser capture</span><span className="pill">Local-first</span></div>
      </section>

      <section className="section" id="features">
        <div className="section-head"><h2>Built as a platform.</h2><p>Shared core, independent clients, clear security boundaries. No promise that protected or unauthorized content can be downloaded.</p></div>
        <div className="grid">{features.map((feature) => <article className="panel" key={feature.title} data-reveal><span className="panel-icon">{feature.icon}</span><h3>{feature.title}</h3><p>{feature.copy}</p><span className="status">{feature.state}</span></article>)}</div>
      </section>

      <footer className="site-footer"><span>YOUR DAWNLOADER · EARLY DEVELOPMENT</span><span><Link href="/privacy">Privacy</Link> · <Link href="/security">Security</Link> · <Link href="/downloads">Platform status</Link></span></footer>
    </main>
  );
}
