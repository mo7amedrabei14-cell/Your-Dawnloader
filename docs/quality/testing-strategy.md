# Testing strategy

1. Unit: URL/credential policy, classification, filenames, quality normalization, queue transitions.
2. Integration: local HTTP fixture for length/type, redirects, Range/ETag, truncation, retry, cancellation, oversized response.
3. Contract: provider schemas, quotas, missing rights, rate limits, malformed metadata.
4. Platform: desktop builds and smoke tests on Windows/macOS/Linux and required architectures; browser extension builds/tests per target.
5. Security/accessibility: hostile fixtures, parser fuzzing, permission tests, keyboard/screen reader checks, RTL and reduced-motion.

Tests must not depend on live music/video providers or download large media from third parties. Use dummy tokens and local fixtures. Inject time/retry behavior when feasible.

Release gate: formatting, lint, unit/integration, typecheck, frontend builds, dependency/security scans, all advertised OS builds, artifact inspection, and manual smoke checklist. Unrun/failed platform checks must be reported.
