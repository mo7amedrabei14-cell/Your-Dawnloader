# ADR 0002: Rust core and Tauri desktop UI

- Status: Accepted
- Date: 2026-10-24

## Decision
Rust owns domain logic, URL policy, detection, download execution, queue semantics, and local storage. Tauri 2 packages the desktop UI, built with React/TypeScript. UI-to-core calls use narrow typed commands.

## Constraints
- Tauri capabilities are restrictive by default.
- The frontend must not have unrestricted shell or filesystem access.
- Do not load arbitrary remote websites inside a privileged desktop webview.
- Use a versioned Native Messaging bridge with strict request validation.
- Mobile builds require separate platform review; desktop assumptions are not copied blindly.
