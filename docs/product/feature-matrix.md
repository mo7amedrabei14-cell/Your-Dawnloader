# Feature inventory

This is a target inventory, not a claim that everything is implemented. Update status only with test/release evidence.

| Area | Capabilities | Acceptance evidence |
|---|---|---|
| Direct HTTP(S) | bounded streaming, safe names, progress, cancel, safe finalization, local history | local fixture server tests |
| Resume | Range, ETag/Last-Modified validation, recovery | 206, 200 fallback, bad validator, truncation tests |
| Detection | URL, MIME, elements, headers, manifests | fixtures and false-positive tests |
| HLS/DASH | parse non-DRM manifests, variants, bounded segments | malformed/oversized fixture suite |
| Browser | toolbar, context menu, candidate capture, native bridge | per-browser build and permissions tests |
| Accounts | sign-in/up, recovery, sessions, devices, export/deletion | OIDC tests and multi-user isolation tests |
| My Downloads | local records, cloud metadata sync, per-device file presence | local store and cross-account tests |
| Search | title/creator, normalized provider results, rights metadata | provider contract tests |
| Quality | actual bitrate, resolution, codec/container, source attribution | representation metadata assertions |
| Conversion | optional local FFmpeg, no fake upscaling, license review | bundled-license validation and integrity tests |
| Queue | concurrency, retry, pause/resume/cancel, priority | deterministic queue integration |
| Storage | local SQLite, server PostgreSQL, migrations, recovery | migration/recovery tests |
| Security | URL/redirect/SSRF policy, safe writes, secret redaction | security regression suite |
| UX | themes, RTL/LTR, keyboard, notifications, reduced motion | accessibility and OS smoke tests |
| Website | docs, releases, sitemap, schema, privacy, security | build and SEO checks |
| Releases | Win x64/x86, macOS Intel/ARM, Linux x64/ARM | native build and smoke tests per target |
| Mobile | share URL, local storage, mobile transfer lifecycle | device/store policy review |
| Operations | dependency updates, SBOM, signatures, provenance | CI evidence and release checklist |

## Status terms
Planned = described; Implemented = code exists; Tested = automated checks passed on named runner; Smoke-tested = human verified on named OS/browser; Released = artifact published with checksums and applicable signing.
