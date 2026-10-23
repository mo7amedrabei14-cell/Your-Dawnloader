import Link from "next/link";
import type { Metadata } from "next";
export const metadata: Metadata = { title: "My Downloads", description: "A private account library for your download history." };
export default function MyDownloadsPage() {
  return <main className="site-shell"><nav className="nav"><Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link><Link className="nav-cta" href="/">Home</Link></nav><section className="account-card"><p className="eyebrow">PERSONAL LIBRARY</p><h1>My Downloads</h1><p className="muted">Your account will keep your download history private and show which records are synced across devices. Download files remain on your devices by default.</p><div className="alert">Account sign-in and sync are not connected yet. This preview does not accept credentials or send user data.</div><div className="empty-state"><div className="symbol">↧</div><h2>Your library will appear here</h2><p>After account sign-in is implemented, you can view completed download records, see which device has a local copy, filter your history, export it, and delete individual records.</p></div></section></main>;
}
