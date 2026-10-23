import Link from "next/link";
import type { Metadata } from "next";
export const metadata: Metadata = { title: "Downloads and platform status", description: "Your Dawnloader platform status and release information." };
const platforms = [
  ["Windows", "x64 and x86", "Build pipeline not yet verified"],
  ["macOS", "Apple Silicon and Intel", "Requires Apple platform build and signing validation"],
  ["Linux", "x64 and ARM64", "Build and runtime smoke tests pending"],
  ["Browsers", "Chrome, Edge, Firefox, Safari", "Separate browser builds and permission checks pending"],
];
export default function DownloadsPage() {
  return <main className="site-shell"><nav className="nav"><Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link><Link className="nav-cta" href="/">Home</Link></nav><section className="account-card"><p className="eyebrow">PLATFORM MATRIX</p><h1>Downloads</h1><p className="muted">No public installers are available yet. We will publish artifacts only after platform builds, integrity checks, and applicable signing validation pass.</p><div className="grid" style={{marginTop:24}}>{platforms.map(([name,arch,status])=><article className="panel" key={name}><h3>{name}</h3><p>{arch}</p><span className="status">{status}</span></article>)}</div></section></main>;
}
