# Roadmap

This is a comprehensive product plan, delivered in dependency order. Feature status must be supported by tests, not labels alone.

## Workstream 0 — Foundation
- [x] Isolated foundation branch.
- [ ] Add monorepo apps/crates and typed contracts.
- [ ] Add CI, dependency policy, and release matrix.
- [ ] Verify development on Windows and builds on Windows/macOS/Linux.

## Workstream 1 — Engine
- [ ] URL policy and redirect validation.
- [ ] Direct-file detection, MIME probe, streaming, limits, safe finalization.
- [ ] Progress, cancellation, retry/backoff, queue and persistent history.
- [ ] Range resume with ETag/Last-Modified validation.
- [ ] Local fixture integration suite.

## Workstream 2 — Desktop
- [ ] Premium responsive UI, Arabic/English, RTL/LTR, theme, keyboard.
- [ ] Queue/history/settings, folder picker, notifications, tray.
- [ ] Tauri capability and CSP review.
- [ ] Installer/smoke tests for every advertised desktop target.

## Workstream 3 — Extensions
- [ ] User-triggered scan, media candidates, permissions.
- [ ] Context menu and optional per-site automatic detection.
- [ ] Versioned Native Messaging host.
- [ ] Chrome/Edge/Firefox/Safari build, install, smoke checks.

## Workstream 4 — Media intelligence/search
- [ ] Real source representations and normalized metadata.
- [ ] Bounded non-DRM HLS/DASH parsing.
- [ ] Official/authorized provider search adapters and rights metadata.
- [ ] Quality selection; optional legal local conversion with license review.
- [ ] Provider contract fixtures and quota controls.

## Workstream 5 — Website/distribution
- [ ] SEO-first docs/site/release pages.
- [ ] Multi-OS artifacts, checksums, SBOM, signing/notarization.
- [ ] Safe updater/rollback, privacy and vulnerability response docs.

## Workstream 6 — Scale/mobile
- [ ] Optional stateless API for catalog/release metadata, not required download proxy.
- [ ] Versioned provider registry and opt-in settings sync after threat review.
- [ ] Android/iOS app paths and store review.
- [ ] Performance/fuzz/accessibility audits and external security review.

Use parallel agents only for isolated files/branches; one integrator owns shared contracts.
