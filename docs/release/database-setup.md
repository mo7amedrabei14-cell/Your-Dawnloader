# Database and API connection

## The important distinction

- `DATABASE_URL` belongs only to the Rust API host and connects the API to PostgreSQL.
- `NEXT_PUBLIC_API_BASE_URL` is a public website build variable that points at the API, for example
  `https://api.example.com`. It must not contain a username, password or database host credentials.
- GitHub Pages hosts only the static website. It cannot execute the Rust API or access PostgreSQL.

## Local testing

From the repository root in Windows PowerShell, run the API helper:

```powershell
.\scripts\dev\start-api.ps1
```

It creates `apps/api/.env` from `.env.example` if needed, starts only a local PostgreSQL container
when the configured database URL points at localhost, and runs `cargo run -p dawn-api`.
On API startup, SQLx applies all pending files from `crates/storage/migrations/` automatically.

Health checks:

```powershell
Invoke-RestMethod http://127.0.0.1:8080/health/live
Invoke-RestMethod http://127.0.0.1:8080/health/ready
```

Start the web UI in a second PowerShell window, also from the repository root:

```powershell
.\scripts\dev\start-website.ps1
```

## Hosted database

Create one empty PostgreSQL database and a dedicated API runtime role. Keep the database reachable
only over TLS and avoid granting the API role superuser or `BYPASSRLS`. Add the full connection URI
to the API provider's server-side secret store under `DATABASE_URL`. Redeploy/restart the API; it
will migrate the schema. Never put this URI in a GitHub Pages variable or commit it to Git.

The database schema migrations are not copied into the website. The website calls the API; the API
is the only component that speaks to PostgreSQL.
