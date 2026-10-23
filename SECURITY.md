# Security Policy

Do not publish exploitable details, tokens, cookies, signed URLs, or user download records in public issues. Use GitHub private vulnerability reporting if enabled. If unavailable, contact the repository owner privately through a channel listed on the owner's GitHub profile.

Please include affected version/commit, impact, minimal reproduction using dummy data, prerequisites, and possible mitigation. Do not access another person's devices, accounts, files, or infrastructure.

## In scope
- URL/redirect handling and cloud-fetch SSRF.
- Arbitrary file writes, path traversal, temporary-file/overwrite issues.
- Credential, cookie, signed URL, browsing history, or media metadata leakage.
- Extension permissions and Native Messaging.
- RCE, dependency/build supply-chain risk, updater signature checks.
- Resource exhaustion from downloads, manifests, retries, or concurrency.

## Out of scope
- Requests to bypass DRM, paywalls, access controls, or provider restrictions.
- Unauthorized testing of third-party services.
- Findings without a credible affected boundary and impact.

## Commitments
- Validate untrusted inputs at boundaries.
- Avoid remote code execution and runtime-downloaded scripts.
- Minimize permissions and retained data; local-first by default.
- Use dependency, secret, static-analysis, and vulnerability checks.
- Verify signed updates when the platform supports them.
- Publish remediation guidance when appropriate.

No software can honestly guarantee complete security.
