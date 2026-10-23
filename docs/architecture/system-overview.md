# System overview

## Target shape
A modular monorepo: shared Rust domain/engine, Tauri desktop UI, WXT browser extension, CLI diagnostics, and Next.js public website.

## Rust modules
- Domain: media candidates, representations, tasks, transitions, provider capabilities, and rights metadata.
- Security: URL policy, credential rejection, bounds, plus a distinct policy for public server-side fetches.
- Detection: direct-file classification, MIME/manifest identification, metadata probing, candidate normalization.
- Download: bounded streaming, queue, progress, retries, resume, integrity, and cancellation.
- Storage: local database, migrations, history/settings, and retention.
- Search/providers: isolated adapters, quotas, capabilities, rights, and normalized metadata.
- Platform: OS keychain, notifications, picker, tray, updater, and platform abstractions.

## Client surfaces
- Desktop: Tauri 2, React, TypeScript.
- Extension: WXT + WebExtension APIs with separate target builds.
- CLI: diagnostics/test harness using the same Rust modules.
- Website: Next.js server-rendered pages for SEO and documentation.

## Optional remote services
Basic direct downloads require no account/backend. Future services may provide release metadata, public provider catalogs, opt-in sync, or explicitly opt-in crash reporting. Do not proxy arbitrary user downloads through cloud infrastructure by default.

## Data flow
1. User pastes URL, chooses browser candidate, or searches title/creator.
2. Normalize and validate input.
3. Detect actual source/media kind and report confidence/capability.
4. Inspect authorized non-DRM metadata/manifests where supported.
5. Offer only real provider representations or clearly labelled local conversions.
6. Queue and stream to a user-selected local destination with limits and progress.
7. Retain minimal task metadata; redact signed URLs/credentials in logs.

## Trust boundaries
Browser content, extension messages, provider data, manifests, remote responses, and local media are untrusted. Native Messaging payloads are revalidated by the desktop core.

## Invariants
- No DRM/paywall/access-control bypass.
- No unauthenticated localhost HTTP bridge.
- No URLs/query strings/cookies/headers/media content in analytics by default.
- Local direct downloads remain available if optional cloud features fail.
