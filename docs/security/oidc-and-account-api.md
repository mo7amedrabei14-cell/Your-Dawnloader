# OIDC sign-in and account API

The API uses OpenID Connect Authorization Code Flow with PKCE S256. Login state is now also bound to a short-lived, HttpOnly, SameSite=Lax browser cookie to reduce login-CSRF/session-swapping attacks. Only a hash of OAuth state is stored server-side. ID tokens are verified against issuer JWKS with RS256 allowlisted, issuer/audience/expiry/nonce checks, issue-time checks and authorized-party checks for multi-audience tokens.

Sessions are opaque random tokens whose hashes are stored server-side. Cookies use HttpOnly for the session and a paired CSRF cookie/header for write operations. State-changing endpoints require a matching CSRF token and the exact configured Origin. Production deployments must use HTTPS and Secure cookies, with SameSite=None when website and API are on separate sites.

Authenticated users can export their account metadata as JSON and request account deletion. Deletion cascades through account profiles, preferences, sessions, devices and synced download history. Exports exclude active session hashes, CSRF hashes, media bytes, local device paths, cookies and provider credentials. Account deletion does not delete any media already stored locally on a device.

Download-history sync is opt-in per user; owner IDs are never accepted from request payloads as proof of authority. This is an implementation foundation, not a production security certification. Before public launch, configure and smoke-test a real OIDC provider, add rate limits/abuse controls, verify production DB TLS, complete device-linking and session-management UX, publish retention/backup-expiry rules, and pass exhaustive cross-account authorization tests.
