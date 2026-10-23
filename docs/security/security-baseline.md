# Security baseline

- Rust safe code by default; document and review any unsafe block.
- Bound every untrusted input and output size.
- Restrictive Tauri capabilities and CSP; never load arbitrary remote sites in privileged UI.
- Extension permissions minimal, user-understandable, just in time.
- No persistent cookie scraping or silent background capture.
- Redact URLs/query strings/headers/cookies/paths from logs.
- No client-embedded provider secrets.
- No telemetry by default; future telemetry must be opt-in and exclude media identity/URLs/content.
- Limits for bytes, timeouts, redirects, manifests, retries, and concurrency.
- Security tests use local fixtures and dummy credentials, not live third-party systems.
- Signed update validation and checksums before installation.

## CI gates to enforce
rustfmt, clippy, unit/integration tests, TypeScript checks, dependency audits, secret scanning, CodeQL/SAST, SBOM, artifact checksums, signing/provenance where available, and threat-model review for new network/provider/bridge capability.

Never claim “100% secure” or “unhackable.” Publish controls, scope, limitations, and incident response.
