# Branch Consolidation — 2026-10-24

This note records the repository consolidation and the remaining dependency work that must not be merged without passing CI.

## Integrated into main

- Account roles, Owner-only user management, login entry points, and SQL migration for the account system.
- Local API and website startup scripts, safe HTTP resume support, and GitHub Pages static website deployment.
- Dependency updates merged after their pull-request CI passed: `base64`, `tower-http`, `sha2`, `@types/node` for the website, TypeScript 7 for the website and extension, `actions/checkout`, and `actions/setup-node`.

## Intentionally not merged

- Desktop TypeScript 7 update: CI failed because the desktop app is missing a declaration for the `./styles.css` side-effect import.
- reqwest 0.13 update: CI failed because the current `rustls-tls` feature no longer exists in reqwest 0.13.
- SQLx 0.9 update: CI failed because the configured `runtime-tokio-rustls` feature no longer exists in SQLx 0.9.
- jsonwebtoken 11 update: GitHub reported merge conflicts against the current main branch.

These updates were closed without merging. Reintroduce them only from a branch based on current `main`, resolve the specific compatibility/conflict issues, and require a fresh full CI pass before merge.
