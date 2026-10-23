# Windows development

## Required tools
- Git for Windows.
- Rust stable via rustup, with rustfmt and clippy.
- Node.js LTS and npm.
- Microsoft C++ Build Tools and WebView2 runtime for Tauri desktop builds.
- Docker Desktop or a local PostgreSQL service for API/database work.

## Start the database
From the repository root:
```powershell
docker compose up -d postgres
```

## Run Rust tests
```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Run the desktop UI in browser dev mode
```powershell
cd apps/desktop
npm install
npm run dev
```

For the native desktop shell, open a second PowerShell and run `npm run tauri dev` in apps/desktop after the Rust toolchain and native build prerequisites are available.

## Run the extension
```powershell
cd apps/extension
npm install
npm run dev
```

## Run the public website
```powershell
cd apps/website
npm install
npm run dev
```

## Current limits
The desktop UI currently implements local URL inspection. The direct downloader engine and account schema exist as separate foundations but are not fully wired to the UI/API yet. Account authentication, sync endpoints, multi-browser packaging, and production release signing must not be considered available until their tests and platform workflows pass.
