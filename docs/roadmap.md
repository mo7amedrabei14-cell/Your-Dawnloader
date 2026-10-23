# Roadmap

The full target architecture is defined now; implementation is delivered in dependency order inside the same system. “Large from day one” means all major product surfaces and contracts are designed up front, not that untested features are falsely declared complete.

## Workstream 0 — Foundation
- [x] Isolated foundation branch.
- [x] Architecture, product, security, and platform planning documents.
- [ ] Monorepo apps/crates and versioned contracts.
- [ ] CI, dependency policy, secret scanning, and release matrix.
- [ ] Verify Windows development and CI builds across Windows/macOS/Linux.

## Workstream 1 — Accounts/API foundations
- [ ] Choose identity provider and document deployment/cost/privacy trade-offs.
- [ ] Implement secure login callback/session flows and account lifecycle.
- [ ] User-scoped API schema, migrations, owner authorization, and rate limits.
- [ ] My Downloads metadata sync, local-only mode, export/deletion, revocation.
- [ ] Cross-account isolation and account lifecycle regression tests.

## Workstream 2 — Shared download engine
- [ ] URL security policy and bounded redirect handling.
- [ ] Direct-file detection, metadata probe, bounded streaming, safe file writes.
- [ ] Queue/history, progress, cancellation, retries/backoff and disk constraints.
- [ ] Safe Range resume with validators.
- [ ] Local fixture integration suite and recovery tests.

## Workstream 3 — Desktop
- [ ] Premium UI, Arabic/English, RTL/LTR, themes, keyboard, reduced motion.
- [ ] Queue/library/history/settings, folder picker, notifications, tray.
- [ ] Tauri capability/CSP review.
- [ ] Installer and smoke tests for every advertised desktop architecture.

## Workstream 4 — Browser extensions
- [ ] User-triggered scan, candidate display, permissions, context menu.
- [ ] Versioned Native Messaging host with strict schema and authorization.
- [ ] Chrome/Edge/Firefox/Safari build, install, and smoke checks.
- [ ] Optional sync tied to account consent; no history upload by default before consent.

## Workstream 5 — Media intelligence/search
- [ ] Real source representations and normalized metadata.
- [ ] Bounded non-DRM HLS/DASH parsing.
- [ ] Official/authorized provider search adapters and rights metadata.
- [ ] Genuine quality selector; optional legal local conversion with license review.
- [ ] Provider contract fixtures and quotas.

## Workstream 6 — Website/distribution
- [ ] Public website and secure account/library screens.
- [ ] SEO/documentation/release pages.
- [ ] Multi-OS packages, checksums, SBOM, signing/notarization.
- [ ] Safe updater/rollback, privacy, security disclosure and retention documents.

## Workstream 7 — Scale/mobile
- [ ] Stateless API scaling, DB indexing/pooling, cache/queue policies and observability.
- [ ] Per-user quota/abuse controls and privacy-respecting metrics.
- [ ] Android/iOS implementations and store reviews.
- [ ] Performance/fuzz/accessibility audits and external security review.

Use parallel contributors only for isolated files/contracts; one integration owner is responsible for shared interfaces.
