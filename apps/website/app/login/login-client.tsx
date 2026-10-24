"use client";

import Link from "next/link";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL?.replace(/\/$/, "");

function authUrl(provider: string): string | undefined {
  if (!API_BASE) return undefined;
  const url = new URL("/auth/oidc/login", API_BASE);
  url.searchParams.set("provider", provider);
  url.searchParams.set("return_to", "/account/downloads");
  return url.toString();
}

export default function LoginClient() {
  return (
    <main className="site-shell">
      <nav className="nav">
        <Link className="brand" href="/"><span className="brand-mark">D</span>Your Dawnloader</Link>
        <div className="nav-links"><Link href="/downloads">Downloads</Link><Link href="/security">Security</Link></div>
      </nav>
      <section className="account-card login-card">
        <p className="eyebrow">YOUR ACCOUNT · YOUR CHOICE</p>
        <h1>Welcome back.</h1>
        <p className="muted">Sign in or create an account with the identity provider. Your Dawnloader creates your app profile after the first verified sign-in; you do not need a separate account form here.</p>
        {!API_BASE && <div className="alert">The public API URL is not configured for this website build. Set the GitHub Actions variable NEXT_PUBLIC_API_BASE_URL before using authentication.</div>}
        <div className="login-methods">
          <a className="login-method google" href={authUrl("google") || undefined} aria-disabled={!API_BASE}>
            <span className="login-method-mark">G</span><span><strong>Continue with Google</strong><small>Sign in or register using Google</small></span><span className="login-arrow">↗</span>
          </a>
          <a className="login-method apple" href={authUrl("apple") || undefined} aria-disabled={!API_BASE}>
            <span className="login-method-mark">●</span><span><strong>Continue with Apple</strong><small>Sign in or register using Apple</small></span><span className="login-arrow">↗</span>
          </a>
          <a className="login-method email" href={authUrl("email") || undefined} aria-disabled={!API_BASE}>
            <span className="login-method-mark">✉</span><span><strong>Continue with email</strong><small>Email and password, if enabled by the identity provider</small></span><span className="login-arrow">↗</span>
          </a>
        </div>
        <div className="login-secondary">
          <a href={authUrl("register") || undefined} aria-disabled={!API_BASE}>Create an account</a>
          <span>·</span>
          <a href={authUrl("reset_password") || undefined} aria-disabled={!API_BASE}>Forgot password?</a>
        </div>
        <p className="login-note">New accounts are assigned <strong>Regular User</strong> by default. VIP access is granted separately after the owner verifies entitlement. Passwords and reset emails are handled by the configured identity provider, never stored in the Your Dawnloader application profile.</p>
      </section>
      <footer className="site-footer"><span>YOUR DAWNLOADER · AUTHENTICATION</span><span><Link href="/privacy">Privacy</Link> · <Link href="/security">Security</Link></span></footer>
    </main>
  );
}
