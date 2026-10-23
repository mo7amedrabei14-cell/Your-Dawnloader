# Your Dawnloader

A cross-platform media discovery and download platform built around a shared Rust core.

> **Status:** early development. This repository is not yet a production-ready downloader. Planned features and supported platforms are documented separately from verified capabilities.

## Product surfaces
- Desktop: Windows x64/x86, macOS Intel/Apple Silicon, Linux x64/ARM64.
- Browser extensions: Chrome, Edge, Firefox, and Safari where platform APIs and packaging permit.
- Media: direct files first; non-DRM HLS/DASH and provider integrations require dedicated tested adapters.
- Search: sources that authorize the relevant use and download.
- Website: releases, documentation, compatibility, changelog, privacy, and security.

Android/iOS are part of the compatibility architecture but require distinct UI, permission, store, signing, and device validation. Mobile extension support is not assumed.

## Non-negotiable principles
1. Never promise every site/media item can be downloaded.
2. Never bypass DRM, paywalls, access controls, or provider restrictions.
3. Basic direct downloads work locally without a cloud account.
4. No media URLs, cookies, browsing history, or file contents in telemetry by default.
5. Treat URLs, redirects, manifests, filenames, provider results, and responses as untrusted.
6. Security, accessibility, localization, tests, and release automation are architecture requirements.

## Repository guide
- docs/architecture — boundaries and decisions.
- docs/product — requirements and feature inventory.
- docs/security — threat model and controls.
- docs/quality — validation strategy.
- docs/release — platform matrix.
- crates — reusable Rust modules.
- apps/desktop — Tauri + React desktop.
- apps/extension — browser extension.
- apps/website — public site.
- apps/cli — diagnostic CLI.

See docs/roadmap.md. A platform is not supported merely because its code compiles.

## Search visibility
The website will follow technical SEO and structured-data guidance. Nobody can guarantee first place in Google or AI-powered search; crawling, indexing, and ranking are controlled by those services.
