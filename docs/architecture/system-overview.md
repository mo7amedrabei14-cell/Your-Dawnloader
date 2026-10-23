# System overview

## Target shape
A modular monorepo with shared Rust domain/engine crates, Tauri desktop UI, WXT browser extension, CLI diagnostics, Next.js public/account website, and a Rust API for accounts and metadata sync.

## Client surfaces
- Desktop: Tauri 2 + React + TypeScript.
- Extension: WXT + WebExtension APIs with separate target builds.
- Website: Next.js server-rendered pages and account experience.
- API: Rust service for identity integration, profile, download metadata sync, account lifecycle, release/catalog metadata.
- CLI: diagnostics and local test harness.

## Rust modules
- Domain: users' scoped records, media candidates, representations, tasks, state transitions, provider capabilities, and rights metadata.
- Security: URL policy, credential rejection, bounds, and separate policy for any public server-side fetcher.
- Detection: direct-file classification, MIME/manifest identification, metadata probing.
- Download: bounded streaming, queueing, progress, retries, resume, integrity, cancellation.
- Storage: local database and server database repositories with separate boundaries; migrations and retention.
- Search/providers: isolated adapters, quotas, rights, and normalized metadata.
- Identity integration: OIDC/OAuth provider adapter, session/token validation, revocation, account lifecycle.
- Sync: versioned idempotent sync protocol for explicitly syncable metadata only.
- Platform: OS keychain, notifications, picker, tray, updater.

## Accounts and API
An account is required for the online website library and cross-device metadata sync. Local direct downloads remain usable when signed out; their records stay local until the user signs in and enables/accepts metadata sync. The API must never require cloud upload of actual media to record a download.

Use a replaceable identity-provider adapter and standards-based OAuth/OIDC. Native desktop clients are public clients: use Authorization Code + PKCE and the system browser, never embed a client secret in the app. Browser and website sessions must follow a separately reviewed web-session design. No credentials are collected inside an untrusted provider page.

## My Downloads semantics
- Local database is authoritative for whether a file exists on a particular device.
- Account library is a cloud-synced activity/metadata view, not proof a file remains on disk.
- Store only minimal fields needed: opaque record ID, account owner, normalized source domain, user-visible title if the user accepts sync, media kind, format/quality metadata, timestamps, status, optional device label, and a local availability marker.
- Do not sync complete signed URLs, query strings, cookies, request headers, auth tokens, or file content.
- User can choose local-only mode; provide per-record deletion, clear-history, export, and account deletion workflows.
- Define conflict handling, deduplication, retention, and cross-device logout/revocation.

## Data flow
1. User pastes URL, chooses browser candidate, or searches title/creator.
2. Normalize and validate the source.
3. Detect actual source/media type and capabilities.
4. Resolve authorized non-DRM metadata/manifest if supported.
5. Show only actual representations or clearly labelled local conversion outputs.
6. Queue and stream to a user-selected local destination.
7. Sync only allowed metadata after user consent and authentication.

## Trust boundaries
Browser page content, extension messages, provider data, manifests, URLs, remote responses, authentication callbacks, sync payloads, and local media are untrusted. Desktop revalidates native messages. API derives account identity from a verified session/token, never from a client-supplied user ID.

## Invariants
- No DRM/paywall/access-control bypass.
- No unauthenticated localhost HTTP extension bridge.
- No sensitive URL/cookie/header/media content in analytics by default.
- Every user-owned API/resource/query/cache entry is explicitly user-scoped and authorization-checked.
- Direct local downloads continue if API/auth/sync are unavailable.
