# OIDC sign-in and account API

The API uses the OIDC authorization-code flow with PKCE S256. Configure a confidential client and an exact callback URL. Server configuration is kept in environment variables; client secrets must never be added to source files or browser bundles.

Required settings: OIDC_ISSUER, OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_AUTHORIZATION_URL, OIDC_TOKEN_URL, OIDC_JWKS_URL and OIDC_REDIRECT_URL. APP_BASE_URL must be the exact website base. Production uses HTTPS, Secure cookies and SameSite=None when website/API are on separate sites; local HTTP uses Secure=false and SameSite=Lax.

Login uses single-use state, nonce and PKCE. Only a hash of OAuth state is stored. ID tokens are verified against the issuer JWKS with RS256 allowlisted, issuer/audience/expiry/nonce checks, issue-time checks, and authorized-party checks for multi-audience tokens. Browser sessions are opaque random tokens whose hashes are stored server-side. Session cookies are HttpOnly and state-changing requests require CSRF cookie/header plus the configured exact Origin.

Every download-history query is scoped to the authenticated principal in a transaction with transaction-local RLS context. Owner IDs are never taken from request bodies. History sync is disabled until each user opts in; records contain minimal metadata, not file bytes, local paths, cookies, credentials, or full signed URLs. Deleting history never deletes local files.

This is an implementation foundation, not a production security certification. Before public launch, configure and smoke-test a real OIDC provider, add rate limits/abuse controls, validate production DB TLS, implement device/session management and export/deletion UX, decide retention/backup expiry, and pass exhaustive multi-user isolation tests.