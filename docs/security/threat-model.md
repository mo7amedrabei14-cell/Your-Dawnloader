# Threat model

## Assets
User media files and destinations; history and settings; cookies/tokens/signed URLs; provider credentials; extension-to-app channel; updater keys and release artifacts; website deployment secrets.

## Threat actors
Malicious sites, providers, redirects, manifests, filenames, malformed media, compromised dependencies/actions, local processes spoofing bridge input, and attackers abusing any future cloud URL resolver.

## Required mitigations
1. URL abuse: approved schemes, reject embedded credentials, bounded redirects, revalidate each hop. Cloud fetchers must resolve and reject private/loopback/link-local/metadata IPs at every hop and defend against DNS rebinding.
2. Resource exhaustion: caps for bytes, redirects, manifest depth/size, segment count, concurrency, retries, and disk/time limits.
3. File writes: sanitize names, ignore separators, app-owned temp files, safe overwrite policy, atomic finalization where supported.
4. Secrets: no auth headers, cookies, signed query URLs, user paths, or media contents in ordinary logs/crash/analytics. Use OS keychain for secrets.
5. Browser privileges: just-in-time permissions, no remote code, restrictive CSP, schema validation on all messages.
6. Native bridge: versioned protocol, strict schema, rate/size caps, supported-operation allowlist; no arbitrary commands.
7. Supply chain: lock dependencies, minimize CI permissions, pin production GitHub Actions by full SHA, scan dependencies, generate SBOM, sign artifacts.
8. Media handling: treat downloads as data, never execute or auto-open; show appropriate file type/integrity metadata.
9. Provider support: official APIs and terms, no DRM bypass, metadata distinct from download authorization.
10. Updater: verify signed metadata/artifacts, fail closed, protect release credentials.

## Boundary note
Local desktop downloads and public server-side URL fetches are different trust models. A cloud resolver must never fetch arbitrary user-supplied URLs with server network privileges.

## Residual risk
Providers change APIs and policies; browser permission models vary; signing requires platform tooling/credentials. Controls reduce risk, not guarantee zero vulnerabilities.
