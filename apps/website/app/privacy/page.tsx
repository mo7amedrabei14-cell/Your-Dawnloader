import Link from "next/link";
import type { Metadata } from "next";
export const metadata: Metadata = { title: "Privacy" };
export default function PrivacyPage() {
  return <main className="site-shell">
    <nav className="nav"><Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link><Link className="nav-cta" href="/">Home</Link></nav>
    <section className="account-card"><p className="eyebrow">PRIVACY PRINCIPLES</p><h1>Files stay yours.</h1>
      <p className="muted">Your Dawnloader is designed local-first. The downloaded media stays on your device by default; an account stores identity and only the metadata needed for opted-in download-history sync.</p>
      <h2>Data minimized by design</h2><p className="muted">The sync contract excludes file bytes, device-specific absolute paths, cookies, provider credentials, and complete signed URLs. The library distinguishes account history from whether a particular device still has the file.</p>
      <h2>Account controls</h2><p className="muted">The target account experience supports sign-out, per-record history deletion and clearing synced history. These actions do not delete locally stored media. Account export/deletion and published retention details remain release requirements before public launch.</p>
      <div className="alert">This is a development-stage privacy overview, not a final legal privacy policy. It must be reviewed against the chosen identity provider, hosting, analytics and data-retention setup before release.</div>
    </section>
  </main>;
}
