# Repository agent instructions

## ADHD-friendly collaboration
1. Lead with the immediate action.
2. Keep each step bounded and numbered.
3. Show current progress and concrete results.
4. Keep visible lists to five items or fewer where practical.
5. End with one clear next action.
6. Do not ask the same question twice. Do not claim completion before checks pass.

## Safe repository operations
- Never commit directly to main. Use a feature branch and PR.
- Inspect files/status before edits; do not overwrite user changes or force-push.
- Never commit tokens, cookies, signed URLs, credentials, or personal media URLs.
- Never execute untrusted media, provider scripts, build scripts, or fixture files outside a sandbox.
- Never test DRM/access-control bypass.
- Record why new dependencies are needed and review their licenses/maintenance.
- Keep generated binaries and user downloads out of git.

## Architecture
- Rust owns domain invariants, URL policy, downloads, queue semantics, and local storage.
- UI calls typed application commands; UI code does not implement network/filesystem policy.
- Extension permissions are minimal and explained, requested at the point of need where possible.
- Do not expose an unauthenticated local HTTP server for extension integration; prefer validated Native Messaging.
- Provider adapters are isolated and state their supported capabilities/rights.
- Separate direct HTTP, HLS/DASH, conversion, metadata search, and provider-specific resolution.
- Never treat a search result as proof of download rights.
- Do not log full URLs by default; query strings can contain signed tokens.
- Public server-side fetchers must validate DNS/IP on every redirect and defend against DNS rebinding.

## UI
- Use React Bits as a reference, not blanket permission to copy code. Verify the exact license before reuse.
- Use GSAP for intentional accessible motion; respect prefers-reduced-motion.
- Use Lenis only where useful on the marketing site, not for core download workflows.
- Keyboard access, focus, contrast, screen-reader semantics, localization, and real RTL/LTR are required.

## Quality gates
- rustfmt; clippy with warnings denied; unit/integration tests.
- Typecheck and build each frontend independently.
- Test release targets on their actual OS/architecture runner.
- Security-sensitive changes require regression tests and threat-boundary notes.
- Never claim “100% secure”; state controls, evidence, and residual risk.
