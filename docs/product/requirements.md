# Product requirements

## Vision
A polished, fast, privacy-first media manager that discovers eligible media, explains available formats/qualities, captures candidates from browsers, and manages reliable downloads across OSes.

## Core journeys
### Paste URL
Classify a direct file, supported non-DRM manifest, provider page, invalid URL, or protected/unavailable source. Show known metadata and real quality choices. Let users choose destination, start, pause/resume/cancel when supported, and see clear progress/errors.

### Browser capture
User grants permission. Extension detects media elements and eligible request metadata through supported browser APIs. It displays candidates without leaking secrets, then downloads an authorized direct URL or sends a versioned request to the desktop Native Messaging host. Desktop validates every request independently.

### Search by title/creator
Search providers return normalized results with source, duration, licensed artwork, and rights/capability metadata. Users select a result and view provider-offered representations or permitted conversion options. Unsupported downloads get an honest explanation and official source link.

### Manage downloads
Queue items, limit concurrency/bandwidth, pause/resume where Range and validators make it safe, retry transient errors, cancel, inspect history, name files safely, detect duplicates, check free disk space, and optionally verify checksums.

## Feature groups
1. Capture: paste, drag/drop, toolbar/context menu, opt-in clipboard watching, open/share handlers where supported.
2. Detection: direct files, audio/video elements, eligible response metadata, non-DRM HLS/DASH, provider metadata.
3. Engine: queue, concurrency, rate limits, cancellation, backoff, resume, progress, integrity.
4. Quality: actual bitrate/resolution/codec/container; optional local conversion only when permitted and licensing is reviewed.
5. Search: title/creator, filters, deduplication, attribution, rights, quotas, official APIs.
6. UX: shortcuts, localization Arabic/English, real RTL, themes, accessibility, reduced motion.
7. Platforms: Windows x64/x86; macOS Intel/ARM; Linux x64/ARM; browser targets; mobile architecture.
8. Website: releases, docs, compatibility, privacy/security, troubleshooting, sitemap/schema/performance.
9. Operations: signed releases, SBOM, dependency policy, opt-in diagnostics, security response.

## Non-functional requirements
Local-first; bounded resources; no sensitive telemetry by default; clear supported/experimental/unsupported/blocked states; accessible, keyboard-operable, RTL/LTR, maintainable, testable.
