# Roadmap

The target architecture is designed up front, while completion status reflects what is implemented and validated—not what is merely planned. CI compilation is necessary but does not substitute for production-provider, database integration, installer, or security tests.

## Workstream 0 — Foundation
- [x] Isolated foundation branch.
- [x] Architecture, product, security, and platform planning documents.
- [x] Monorepo apps/crates and shared, versioned API contracts.
- [x] CI for Rust workspace, website, desktop frontend, extension frontend, and Tauri native compile.
- [x] CI compilation and Rust tests across Windows, macOS, and Linux.
- [ ] Add automated secret scanning, dependency/license policy checks, and release workflows.
- [ ] Build and smoke-test signed installers on every advertised OS/architecture.

## Workstream 1 — Accounts/API foundations
- [x] Choose Logto as the first OIDC broker candidate and document provider setup, role bootstrap and password recovery responsibilities; real provider configuration remains pending.
- [x] Implement OIDC authorization-code callback, PKCE, state/nonce validation, JWT/JWKS checks, hashed sessions, and CSRF protection.
- [x] Add account schema/migrations, owner-scoped transactions, role/status fields, and RLS for user profile, preferences, devices, download records, and device presence.
- [x] Implement account export and deletion endpoints and web controls.
- [x] Implement web My Downloads metadata library, user opt-in for sync, device pairing, device list/revocation, desktop keychain-backed device credentials, and owner-only user administration API/UI.
- [x] Add local metadata-only sync outbox, opt-in backfill, atomic queue clearing when local history is deleted, retry backoff, and cross-platform build tests.
- [ ] Run live end-to-end tests against PostgreSQL and a real OIDC provider, including owner bootstrap, role/status changes, multi-user isolation and account/device lifecycle scenarios.
- [ ] Add API rate limits/abuse controls, production DB TLS verification, recovery/session-management UX, and retention/backup-expiry policy.

## Workstream 2 — Shared download engine
- [x] URL parsing/security policy foundation and direct-file classification.
- [x] Direct-file streaming foundation, progress events, cancellation, local SQLite history, and bounded concurrent tasks.
- [x] Retryable local metadata outbox for opted-in account history.
- [ ] Harden bounded redirect handling, DNS/private-network protections, safe file creation, and disk constraints with integration tests.
- [ ] Safe Range resume using validators and crash recovery.
- [ ] Expand local fixture, malformed-response, filesystem-failure, and recovery suites.

## Workstream 3 — Desktop
- [x] Tauri + React application foundation and cross-platform native-crate CI.
- [x] Account connection flow, system-browser approval, OS credential-store integration, and cloud-history opt-in controls.
- [ ] Complete product-grade UI, Arabic/English localization, RTL/LTR, themes, keyboard accessibility, and reduced-motion behavior.
- [ ] Finish queue/library/settings UX, folder picker, notifications, tray behavior, and accessibility review.
- [ ] Review Tauri capabilities/CSP and build/smoke-test signed installers across every advertised architecture.

## Workstream 4 — Browser extensions
- [x] Extension frontend build in CI.
- [ ] User-triggered scan, candidate display, minimal permissions, and context-menu flow.
- [ ] Versioned Native Messaging host with strict schema and authorization.
- [ ] Chrome/Edge/Firefox/Safari packaging, installation, and smoke checks.
- [ ] Connect extension history only through account opt-in; no upload without consent.

## Workstream 5 — Media intelligence/search
- [ ] Normalized source representations and metadata.
- [ ] Bounded non-DRM HLS/DASH parsing.
- [ ] Official/authorized provider search adapters and rights metadata.
- [ ] Genuine quality selector; optional legal local conversion with license review.
- [ ] Provider contract fixtures, quotas, and abuse controls.

## Workstream 6 — Website/distribution
- [x] Public-site foundation and account/library screens.
- [x] Account connection and connected-device management screens.
- [x] Document account roles, OIDC broker responsibility, local/hosted database setup, and local API startup.
- [ ] Complete compatibility, release, support, and SEO pages.
- [ ] Multi-OS packages, checksums, SBOM, signing/notarization.
- [ ] Safe updater/rollback, final legal/privacy notices, security disclosure, and published retention policy.

## Workstream 7 — Scale/mobile
- [ ] Stateless API scaling, DB indexing/pooling, cache/queue policies, and observability.
- [ ] Per-user quotas/abuse controls and privacy-respecting metrics.
- [ ] Android/iOS implementations, device validation, and store reviews.
- [ ] Performance, fuzz, accessibility, and external security audits.

## Validation status
- The CI workflow has passed for the current implementation on Windows, macOS, and Linux.
- CI does not verify a live OIDC identity provider, deployed PostgreSQL configuration, browser cookie behavior in production, data deletion at a deployed provider, signed installers, or external security readiness. Do not call the project production-ready until those checks are completed.

Use parallel contributors only for isolated files/contracts; one integration owner is responsible for shared interfaces.
