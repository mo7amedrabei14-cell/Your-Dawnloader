# Accounts and My Downloads

## Product contract
Each person has an individual account. An authenticated person may see only their own profile, settings, devices, synced download records, and associated metadata. Administrative access is exceptional, least-privileged, justified, and audited.

## Authentication
- Identity is handled through a replaceable OAuth 2.0 / OpenID Connect provider adapter.
- Desktop and mobile are public clients. Use Authorization Code + PKCE through the system browser; do not ship client secrets or collect provider passwords in embedded webviews.
- Browser website sessions have a separate design: Secure, HttpOnly, SameSite cookies where appropriate, CSRF protections, strict redirects, session rotation, expiry, revocation, and re-authentication for sensitive actions.
- The provider must support verified identity/email as required for recovery and anti-abuse, secure password recovery if passwords are used, MFA/passkeys where supported, rate limits, and suspicious-login handling.
- Never invent an authentication system by storing plaintext passwords or creating custom cryptography. Select and document the actual provider before production implementation.
- Sign-out revokes server sessions and clears local access/refresh credentials from OS secure storage. Native client tokens are stored in OS keychain facilities, not browser localStorage or plaintext config.

## My Downloads
The account page displays user-scoped download history and its sync provenance. It is a metadata library, not cloud media storage:
- A local record may be synced after opt-in and login.
- A synced remote record does not prove the original file exists on the current device.
- Local paths are device-specific and are not uploaded.
- Full source URLs and query strings are not synced by default. Keep source domain and a safe provider/item ID when available.
- The user can mark records local-only, remove individual records, clear synced history, export account data, revoke devices, and delete their account.
- If a source requires a short-lived signed URL, it is fetched afresh through an authorized integration; the account library must not retain token-bearing URLs.
- The system must not imply that historical playback/download grants ongoing rights.

## Sync protocol
- Versioned request/response schema; idempotency key per operation; optimistic concurrency/versioning.
- The server derives the user ID from verified identity, never from a request body/URL parameter.
- Each list/get/update/delete operation applies owner authorization at the repository boundary.
- Paginated responses, bounded payloads, per-user quotas, and rate limits prevent noisy-neighbor exhaustion.
- Offline local changes are queued locally and reconciled with deterministic conflict rules.
- Account deletion revokes credentials, schedules deletion of cloud metadata and profile data, and records minimal non-content audit evidence where legally required.
- Define retention and backup-expiry windows clearly before production launch.

## Non-goals for account v1
- No default uploading or cloud storage of songs/videos.
- No syncing cookies, provider sessions, credentials, complete signed URLs, local absolute paths, or media content.
- No cross-account sharing of download records.
- No promise that a record can be downloaded again.
