# Local development quick start (Windows / PowerShell)

## Requirements

- Git
- Node.js 22 or newer with npm
- Docker Desktop (running)
- Rust stable toolchain (rustup / cargo)

## Start the API and local PostgreSQL

From the repository root, run:

```powershell
.\scripts\dev\start-api.ps1
```

On first run it creates the ignored local file `apps/api/.env` from the example.
It starts the local PostgreSQL container only when `DATABASE_URL` points to localhost; if you set an existing remote PostgreSQL URL in `apps/api/.env`, it uses that connection and does not start a local database container. The script imports the environment variables and runs `cargo run -p dawn-api`.
The API applies SQLx migrations automatically from `crates/storage/migrations/`
when it starts. Do not paste migration SQL manually when using this startup script.

Confirm the health routes in a second PowerShell window:

```powershell
Invoke-RestMethod http://127.0.0.1:8080/health/live
Invoke-RestMethod http://127.0.0.1:8080/health/ready
```

The example OIDC issuer is intentionally fake (`identity.example.invalid`). The API
can be launched with the example environment, but real account sign-in cannot work
until a genuine OIDC provider is configured. Local PostgreSQL credentials in
`compose.yaml` are disposable development credentials and must never be exposed
to the public internet.

## Start the website

In another PowerShell window:

```powershell
.\scripts\dev\start-website.ps1
```

Open `http://127.0.0.1:3000`. This starts the web UI, not the API.

## Database migration files

- `crates/storage/migrations/0001_account_library.sql`: user accounts, devices,
  download metadata, indexes and row-level policies.
- `crates/storage/migrations/0002_oidc_sessions_and_profile.sql`: OIDC login state,
  hashed browser sessions, and verified profile fields.
- `crates/storage/migrations/0003_device_link_requests.sql`: temporary device-link
  requests and their constraints/indexes.

SQLx records migration versions and skips migrations already applied. Back up a
database before making schema changes. Prefer creating a new numbered migration
rather than editing one that may already have run in a shared environment.

## Common problems

- Docker not started: open Docker Desktop and wait for the engine before rerunning when using the local PostgreSQL configuration.
- Existing remote PostgreSQL: set its connection URL in `apps/api/.env`; confirm the user can run migrations and that TLS is configured as required by that provider.
- Port 5432 busy: inspect existing local PostgreSQL/container use; do not delete the
  Docker volume to solve this without first confirming it contains no needed data.
- Port 8080 busy: stop the other local API or change `API_BIND_ADDRESS` in
  `apps/api/.env` and update health-test URLs.
- Rust dependencies fail: run `rustup update stable` then rerun the script.
- Login fails: the example OIDC host is a placeholder; this is expected until
  a real provider and callback URL are configured.
