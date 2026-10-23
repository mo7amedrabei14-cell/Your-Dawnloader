# Account security and data lifecycle

## Required identity controls
- OIDC authorization code flow; PKCE for desktop/mobile public clients; exact redirect allowlist and state/nonce validation.
- Do not embed client secrets in desktop/mobile/extension binaries.
- Browser web sessions use Secure, HttpOnly, SameSite cookies where appropriate, CSRF protection, rotation and revocation.
- Short-lived access tokens; refresh-token rotation/reuse detection where the chosen provider supports it.
- Re-authentication or step-up authentication for password/email changes, device revocation, export, and account deletion.
- Enumeration-resistant login/recovery responses, rate limits, verification flows, and abuse controls.
- Secrets stored in OS keychain on desktop; no tokens in URLs, analytics, logs, or localStorage by default.

## Multi-user authorization
- Derive principal from a verified session/token on the server.
- Ignore client-supplied owner IDs as proof of identity.
- Authorization at service and repository boundaries, with explicit owner-scoped methods.
- Consider PostgreSQL Row-Level Security as defense in depth only if connection pooling/transaction-local tenant context and role bypass are tested correctly.
- Every cache key, queue item, export, background task and sync cursor must be scoped to a verified user.
- Rate-limit by account and IP as appropriate; apply per-account queue/concurrency/quota controls.
- Admin/support access is exceptional, least-privileged, logged, and policy-governed.

## Data minimization and deletion
- Separate local download files from cloud account metadata.
- Never sync absolute device paths, cookies, credentials, media bytes, or full signed URLs by default.
- Provide account data export, per-record delete, clear cloud history, revoke sessions/devices, and account deletion.
- Publish retention schedules including backup expiry before public launch.
- Avoid retaining token-bearing URLs in crash dumps or error tracking.

## Validation requirements
- Two or more test users with deliberately similar record IDs.
- Try cross-user list/get/update/delete, guessed IDs, export, sync cursor replay, pagination, cache-key collisions, stale session, revoked token, and background job retry.
- Verify the ordinary API database role cannot bypass any Row-Level Security used as a safety boundary.
- Test account deletion while offline clients reconnect later; deleted cloud records must not be re-created from stale sync state without an explicit recovery rule.
- Test backup restore, session revocation, and identity-provider outage behavior.
