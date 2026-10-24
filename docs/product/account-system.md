# Your Dawnloader account system — source of truth

This document is the single planning source for identity, profiles, roles, user administration,
database ownership, and release gates. It distinguishes implemented API behavior from provider
configuration that still requires an operator.

## Architecture decisions

- The Next.js website is a static GitHub Pages site. It never connects directly to PostgreSQL.
- The Rust API is the only service allowed to connect to the application database.
- PostgreSQL schema changes live in numbered SQLx files under `crates/storage/migrations/`.
  `Storage::migrate()` applies them on API startup. Do not paste migrations manually when the
  API manages the same database.
- Use one OpenID Connect identity broker as the app's identity authority. Configure Google,
  Apple, and email/password sign-in in that broker. It owns password hashing, verification
  emails, forgotten-password flows, reset tokens, and social-account linking. The Rust API
  receives verified OIDC claims and owns app profiles, roles, billing state and product data.
- Never store a Google password, Apple password, OIDC client secret, or identity-provider
  refresh token in `app_users`.
- A first successful OIDC callback creates a database user automatically. New accounts receive
  `regular_user` unless the verified email matches the server-side `OWNER_EMAIL` bootstrap
  setting. A display name is never proof of ownership.
- The API's role/status checks are authoritative; hiding UI buttons is not authorization.

## Four roles

| Stored role | Display label | Assignment | Authority today |
|---|---|---|---|
| `owner` | Mohamed Rabea | First authenticated account with verified email equal to `OWNER_EMAIL` | Full account-management authority in this API |
| `admin` | Admin | Explicit owner assignment only | Reserved: no extra administrative routes granted yet |
| `vip_user` | VIP User | Explicit owner assignment after purchase verification | No paid features granted until entitlements are specified |
| `regular_user` | Regular User | Default for every new user | Standard account features |

The fourth role was not named in the request, so `admin` is the proposed placeholder. It deliberately
receives no extra permission until the owner decides what it can do. VIP status must eventually be
changed from a verified payment/webhook flow, not from a browser form.

## Account lifecycle

1. Visitor clicks Google, Apple, email sign-in, or create-account on `/login`.
2. The identity broker signs the person in (or creates their identity for first-time signup).
3. The API verifies the OIDC issuer, audience, signature, nonce, state, PKCE and verified email claims.
4. The API creates the local application account if missing; its default role is `regular_user`.
5. If the verified email matches `OWNER_EMAIL`, the API bootstraps the owner role and the configured
   display name `Mohamed Rabea`.
6. The owner lists accounts and may update non-owner profiles, roles and active/suspended status.
7. Forgot-password and password changes are handled by the identity broker, because social identities
   do not have a password owned by this application. A user who chose Google or Apple resets the
   password at that provider; an email/password user follows the broker's verified reset flow.

## API contract

- `GET /api/v1/me`: authenticated user, current role and profile.
- `GET /api/v1/admin/users?limit=50`: owner-only user list (limit capped by the API).
- `PATCH /api/v1/admin/users/{id}`: owner-only update of non-owner role, display name, locale or status.
- Account deletion/export, device management and download history remain owner-scoped per user.
- Only the owner may call user-administration endpoints. The reserved `admin` role is not an alias
  for owner and does not inherit owner rights.

## Database setup

### Local PostgreSQL

From the repository root on Windows PowerShell:

```powershell
.\scripts\dev\start-api.ps1
```

On first run, the script copies `apps/api/.env.example` to the ignored `apps/api/.env`, starts the
local PostgreSQL container only when `DATABASE_URL` targets localhost, and runs the API. SQLx
migrations are applied automatically.

To inspect migrations, open `crates/storage/migrations/`. Current application migrations are:
- `0001_account_library.sql`
- `0002_oidc_sessions_and_profile.sql`
- `0003_device_link_requests.sql`
- `0004_account_roles_and_status.sql`

### Hosted PostgreSQL

1. Create one empty PostgreSQL database on your provider (PostgreSQL 15+ is a sensible target).
2. Create a dedicated runtime database user; do not use a provider's master/admin account for the
   deployed API. Ensure that the runtime user can apply migrations during initial setup, or run a
   controlled migration job with a migration-only database user.
3. Put its TLS-enabled connection URI in the API host's secret named `DATABASE_URL`. It is never
   a website environment variable.
4. Set API server environment variables in the API host; never commit real `.env` files.
5. Deploy the API. On startup it connects to that database and applies migrations; check
   `/health/live` and `/health/ready`.
6. Set the GitHub Pages Actions variable `NEXT_PUBLIC_API_BASE_URL` to the API's HTTPS origin only.
   This value is public; it is not a place for a database URI or credentials.

The API's `DATABASE_URL` is the database connection. The website's `NEXT_PUBLIC_API_BASE_URL` is
the public API URL. They are different values and must never be swapped.

## Provider configuration — still required from the owner

- Create an OIDC application in the identity broker.
- Configure the broker's Google connector with Google OAuth client ID/secret.
- Configure Apple Sign In using an Apple Services ID, Team ID, Key ID and private key. Apple web
  sign-in requires periodic client-secret rotation; keep its private key server-side.
- Enable email/password registration and a working verified-email/password-reset template in the
  broker. Configure its email provider and sender domain.
- Set the OIDC issuer, client ID/secret, authorization/token/JWKS URLs and exact callback URL in the
  API host. Register `https://YOUR_API_HOST/auth/oidc/callback` in the broker. Do not put secrets in
  GitHub Pages variables.
- Set `OWNER_EMAIL` to the exact email that the identity broker will assert as verified, and
  `OWNER_DISPLAY_NAME=Mohamed Rabea`. Never use display-name matching as an owner grant.
- Set `APP_BASE_URL` to the site's full base URL and match CORS/cookie settings.

Logto is the selected OIDC-broker implementation target because its sign-in experience supports provider-specific
social sign-in and first-screen options, including password reset. Google, Apple and email/password live in Logto
while the Rust API remains the application's data/API backend. The real Logto tenant and provider credentials
still need to be configured by the owner.

## GitHub Pages and browser cookies

The current project URL and an unrelated API host are cross-site. Modern browser privacy settings may
block third-party cookies, even if the API allows credentialed CORS. For a reliable cookie-session
production setup, use a custom domain for the website and place the API on a subdomain of that same
domain, or deliberately redesign the browser auth flow and validate it in supported browsers. Do not
declare production auth working just because the static website deploys.

## Release gates

- [x] Static site publishes from GitHub Actions.
- [x] Local API startup script and migration documentation.
- [ ] Apply migration 0004 in a disposable PostgreSQL integration database and test default role.
- [ ] Test owner bootstrap only for verified `OWNER_EMAIL`.
- [ ] Test owner-only administration, invalid role/status rejection, suspended-session/device revocation,
      and cross-account isolation.
- [ ] Configure and test Google, Apple, email/password registration, verification and password reset.
- [ ] Configure hosted PostgreSQL and a reliable same-site web/API origin, then test cookies.
- [ ] Add rate limiting, abuse controls, auditing, backup/restore, and security review before public launch.
