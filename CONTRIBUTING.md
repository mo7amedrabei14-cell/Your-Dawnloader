# Contributing

1. Read AGENTS.md and relevant architecture decisions.
2. Define user impact and acceptance criteria in an issue.
3. Create a feature branch from current main.
4. Keep the change reviewable and include tests.

Pull requests must explain behavior and trust-boundary changes, include test results, update docs/platform status, disclose new dependencies and licenses, and contain no secrets or personal media data. Do not merge failing required checks.

Rust changes should be idiomatic, clippy-clean, explicit about limits, and avoid panics for expected failures. TypeScript must be strict and validate runtime inputs. UI must support keyboard navigation, reduced motion, localization, RTL, and accessible focus. Provider adapters must document official API/auth/rights and must not bypass DRM.

A release is supported only after required platform builds, artifact inspection, and smoke tests pass.
