# OIDC sign-in, desktop pairing, and account API

## Identity and browser session
The API implements OpenID Connect Authorization Code Flow with PKCE S256. Login state is single-use and server-side storage keeps only its hash; a short-lived HttpOnly/SameSite=Lax cookie binds the callback to the browser that started login. ID tokens are verified using issuer JWKS with RS256 allowlisted, issuer/audience/expiry/nonce checks, issue-time checks, and authorized-party checks for multi-audience tokens.

Browser sessions are opaque random tokens whose hashes are stored server-side. The session cookie is HttpOnly. State-changing web requests require a matching CSRF cookie/header pair and the exact configured website Origin. Production requires HTTPS and Secure cookies; SameSite=None is needed if the website and API are cross-site. Keep the website and API within a compatible first-party cookie arrangement where possible.

## Desktop device approval
1. The desktop asks the API for a device authorization request. The API returns a short-lived user code and verification URL; the high-entropy device code remains in the native Rust process and is never exposed to React.
2. The user signs into the website and explicitly approves the code. Approval requires an authenticated browser session, CSRF validation, and a matching website Origin.
3. The server stores only a hash of the device code. The device becomes usable only after approval and is associated with one authenticated account.
4. The desktop saves the resulting device credential only in the operating-system credential store through the keyring library. The credential is not written to the local history database or returned to frontend JavaScript.
5. The user can list linked devices and revoke device access from the account library page. Server authorization rejects revoked devices even if a local credential remains.

Pending device codes expire after ten minutes. Cancellation in the desktop UI stops local polling; an unapproved server request can remain until expiry. Add production rate limits and abuse monitoring before public release.

## Download-history sync and local privacy
Download history sync is off by default per account. The web page and desktop setting both control the account preference. When the user opts in, the desktop can queue existing local history metadata and upload metadata for new completed downloads.

The local retry outbox contains only a stable record ID, source domain, display name, media type/container, completion timestamp, queue time, and attempt count. It excludes media bytes, local file paths, full source URLs, cookies, authorization headers, and provider credentials. Failed uploads use capped exponential backoff so one failed record does not block the entire queue. Removing local history also removes its pending outbox entry transactionally; cloud history is a separate account resource and is not automatically deleted by a local-history removal.

Turning cloud sync off clears the local pending queue but intentionally leaves previously synced cloud records in the account until the user removes them or deletes the account. Re-enabling sync can queue current local history again. Removing synced records or deleting the account affects cloud metadata only; it does not delete media files on any device.

## Account data export and deletion
Authenticated users can download a JSON export of profile/account metadata, preferences, known devices, and synced download history. It excludes active session/CSRF hashes, media bytes, local paths, cookies, and provider credentials. Account deletion removes account profiles, preferences, sessions, linked devices, and synced download metadata through database cascades. Media files stored on a computer remain untouched.

## Required deployment configuration
The API requires server-side environment values: `DATABASE_URL`, `OIDC_ISSUER`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_AUTHORIZATION_URL`, `OIDC_TOKEN_URL`, `OIDC_JWKS_URL`, `OIDC_REDIRECT_URL`, and `APP_BASE_URL`. Configure a real confidential OIDC application and register the exact callback URL. The example issuer in `.env.example` is intentionally invalid and cannot be used to sign in. Set `NEXT_PUBLIC_API_BASE_URL` to the public API origin in the website deployment. Never place the client secret in the website's public environment or desktop binary.

PostgreSQL migrations run on API startup. In production, use a dedicated database role that does not own the protected tables and does not have BYPASSRLS; verify TLS and connection limits. Owner-scoped RLS is defense in depth and does not replace API authorization.

## Production blockers
This implementation has passed repository CI but is not a security certification or production release. Before public launch:
- Configure and smoke-test a real OIDC provider, including callback/cookie behavior on the chosen deployed origins.
- Add API rate limiting, abuse protection, and suitable login/device-code attempt limits.
- Run PostgreSQL integration tests for cross-account access, migrations, export/deletion cascades, revocation, and concurrent device approval.
- Verify production database TLS, backup/restore, retention/expiry, log redaction, and incident-response practices.
- Complete account recovery and session/device management UX, legal privacy disclosures, signed installers, and an external security review.
