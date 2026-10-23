# ADR 0001: Cross-platform product from the beginning

- Status: Accepted
- Date: 2026-10-24

## Decision
Use a monorepo with a shared Rust core and independent application surfaces. Target Windows x64/x86, macOS Intel/Apple Silicon, and Linux x64/ARM64 from the beginning. Build shared-source extensions for Chromium browsers and Firefox, with a Safari-specific packaging/validation path.

Android/iOS remain architectural targets with distinct UI/platform implementation, review, signing, and physical-device validation. They must not be called release-ready merely because the framework supports mobile.

## Rationale
Shared invariants should not fork by OS. Permissions, signing, packaging, and browser APIs differ, so shared source still requires platform-specific validation.

## Consequences
CI requires platform-specific runners. Some release artifacts require signing credentials. Each platform must be labelled planned, build-tested, smoke-tested, signed/notarized, or released.
