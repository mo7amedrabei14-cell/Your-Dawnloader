# Product requirements

## Vision
A polished, fast, privacy-first media manager with individual accounts, browser capture, media discovery, honest quality options, reliable downloads, and cross-platform clients.

## Accounts and personal library
Every person can create an account, sign in, sign out, recover access, manage sessions, and control privacy/sync. The web account area includes profile, security, linked devices, data export/deletion, and **My Downloads**.

My Downloads lists download records already completed/managed by the user. It must clearly distinguish:
- file currently present on this device;
- record synced from another device, with no local file available here;
- failed/cancelled task;
- provider/source link that is still available;
- metadata that is stale or unavailable.

Actual media files stay local by default. Cross-device history syncs minimal metadata only. Cloud media storage is a separate opt-in feature, not silently implied by an account. A download record must never falsely imply cloud ownership or re-download rights.

## User journeys
### Paste URL
Classify direct file, supported authorized non-DRM manifest, provider page, invalid URL, or protected/unavailable source. Display known metadata and genuine quality choices. Download locally with progress, errors, cancellation and resume where supported.

### Browser capture
User grants permission. Extension detects eligible candidates via supported browser APIs, shows source metadata without leaking secrets, then sends a validated candidate to desktop over Native Messaging or downloads an authorized direct URL. Desktop revalidates.

### Search by title/creator
Authorized search providers return normalized results with source, duration, artwork where licensed, and rights/capability metadata. User selects a result and sees provider-offered representations or legal local conversion outputs. Unsupported downloads show honest explanations and official source links.

### Account sync
Signed-in users can choose to sync download history and selected preferences. Upload only explicit sync fields; support deduplication, conflict handling, per-record deletion, full export, account deletion, revocation, and local-only mode. Offline changes sync safely when connectivity returns.

### Download management
Queue items, set concurrency/bandwidth limits, pause/resume when Range and validators make it safe, retry transient errors, cancel, inspect history, safely name files, detect duplicates, and verify optional checksums.

## Functional groups
1. Accounts: registration/login, verified email where applicable, recovery, OAuth/OIDC, session management, logout/revocation, account export/deletion, privacy preferences.
2. Library: My Downloads, per-device local availability, filters/search, status, per-record delete, clear history, local-only mode.
3. Capture: paste, drag/drop, browser toolbar/context menu, opt-in clipboard monitoring, OS open/share handlers where supported.
4. Detection: direct files, audio/video elements, eligible response metadata, authorized non-DRM HLS/DASH, provider adapters.
5. Engine: queue, concurrency, limits, cancellation, backoff, safe resume, progress, integrity.
6. Quality: actual bitrate/resolution/codec/container; optional conversion only with legal source and dependency license review.
7. Search: title/creator, filters, dedupe, attribution, rights metadata, quotas, official APIs.
8. Platforms: Windows x64/x86; macOS Intel/ARM; Linux x64/ARM; browser targets; mobile-specific implementation.
9. Website: marketing/docs/releases/account/library/privacy/security/support, SEO, accessible performance.
10. Operations: signed releases, SBOM, dependency policy, opt-in diagnostics, support and incident response.

## Non-functional requirements
Local-first; no sensitive telemetry by default; bounded resources; clear supported/experimental/blocked status; accessible keyboard UX, localization, real RTL/LTR; server-enforced per-account authorization; export/deletion and documented retention.
