# Your Dawnloader

A cross-platform media discovery, download management, and account platform built around a shared Rust core.

> **Status:** foundation in progress. This is not yet a production-ready downloader or account service. Planned features are tracked separately from features proven by tests and releases.

## Product surfaces
- **Accounts and web app:** individual accounts, secure sign-in, profile/security settings, a cloud-synced *My Downloads* activity/library page, and account data export/deletion.
- **Desktop:** Windows x64/x86, macOS Intel/Apple Silicon, Linux x64/ARM64.
- **Browser extensions:** Chrome, Edge, Firefox, and Safari where APIs and packaging permit.
- **Media:** direct files first; authorized non-DRM HLS/DASH and provider integrations through isolated adapters.
- **Website:** product, downloads, docs, compatibility, releases, privacy, security, and support.

Android/iOS are part of the architectural plan but require separate UI, permission, store-policy, signing, and device validation. Mobile extension support is not assumed.

## Important product behavior
- Every user has a private account.
- **My Downloads** lists completed download records associated with that account and, when a desktop is connected, can show which items are present on that device.
- By default, media files stay on the user's device. Account sync stores minimal download metadata, not copies of the media. Cloud file storage, if ever added, is a separate explicit opt-in feature with quotas and rights controls.
- Users must be able to inspect/export and request deletion of their account data.
- Direct downloads do not require a paid plan or cloud media-processing service.

## Non-negotiable principles
1. Never promise that every site/media item can be downloaded.
2. Never bypass DRM, paywalls, access controls, or provider restrictions.
3. Treat every URL, redirect, manifest, filename, provider result, account field, and remote response as untrusted.
4. No media URLs, cookies, browsing history, or file contents in analytics by default.
5. Server-side authorization must enforce per-user ownership on every account resource.
6. Security, accessibility, localization, testing, and release automation are architectural requirements.

## Repository guide
- docs/architecture — system boundaries and decisions.
- docs/product — requirements, accounts, and feature inventory.
- docs/security — threat model and security controls.
- docs/quality — validation strategy.
- docs/release — platform matrix.
- crates — reusable Rust modules.
- apps/desktop — Tauri + React desktop.
- apps/extension — browser extension.
- apps/website — public site/account UI.
- apps/api — account, sync, and public metadata API.
- apps/cli — diagnostics.

See docs/roadmap.md. A platform is not supported merely because its code compiles. First place in Google or AI-powered search cannot be guaranteed.
