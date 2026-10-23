# GitHub Pages deployment

## What GitHub Pages hosts

The public website is exported as static HTML, CSS and JavaScript and deployed from
`apps/website/out`. The project site URL is:

`https://mo7amedrabei14-cell.github.io/Your-Dawnloader/`

GitHub Pages does not run the Rust API or PostgreSQL. The API must be deployed to
a separate Rust-capable service before account login, device pairing and cloud
history sync can work. With no `NEXT_PUBLIC_API_BASE_URL` repository variable,
the public pages still render, while the account UI explains that the API origin
must be configured.

## One-time GitHub setting

After merging the Pages workflow into `main`:

1. Open repository **Settings → Pages**.
2. Under **Build and deployment → Source**, select **GitHub Actions**.
3. Open **Actions → Deploy website to GitHub Pages** and verify the build and deploy jobs.

Pushes to `main` that change `apps/website/**` automatically rebuild and deploy.
Pull requests run a static build/type-check but do not deploy. The workflow can also
be run manually from the Actions tab.

## Public website configuration

If a separately deployed API is ready, add a repository **Actions variable** named
`NEXT_PUBLIC_API_BASE_URL` (not a secret), containing the API's HTTPS origin only,
without a trailing slash. This value is public and becomes part of the website's
client-side JavaScript.

Do not put `OIDC_CLIENT_SECRET`, `DATABASE_URL`, API tokens, or database credentials
in a `NEXT_PUBLIC_*` variable. These belong only in server-side API environment
secrets.

For the API, set `APP_BASE_URL` to the full Pages base URL
`https://mo7amedrabei14-cell.github.io/Your-Dawnloader`, and configure the OIDC
callback URL to the API's own `/auth/oidc/callback` endpoint. Configure CORS and
cookie attributes for the exact website/API deployment origins. Cross-site cookie
restrictions may prevent account sessions from working reliably when Pages and API
are on unrelated domains; validate sign-in in real browsers before launch. A custom
domain and same-site API arrangement may be required.

## Static-export limitations

GitHub Pages serves static files only. Request-time Next.js server rendering,
API routes, server actions, and server-set security headers are not available here.
The account pages can load as a client-side shell, but authenticated functions
depend on the separately deployed API. These pages are not proof that account login
or cloud storage is operational. The Privacy and Security pages explicitly describe
this as an early-development project.

## Local API and database

From the repository root, start the disposable local PostgreSQL database:

```powershell
docker compose up -d postgres
```

In a PowerShell window, set the local environment required by the API using the
values from `apps/api/.env.example` (the example OIDC issuer is deliberately
invalid and cannot authenticate). Then run:

```powershell
cargo run -p dawn-api
```

On startup, the API connects to `DATABASE_URL` and applies SQLx migrations
automatically from `crates/storage/migrations/`. Do not paste the migration SQL
into the database console when running this API unless you deliberately intend to
manage migrations manually. Confirm local readiness at
`http://127.0.0.1:8080/health/ready`.

The example PostgreSQL credentials are disposable local development values only.
Never expose this database or these credentials to the public internet.
