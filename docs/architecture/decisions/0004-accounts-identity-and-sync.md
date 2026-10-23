# ADR 0004: Individual accounts and metadata sync

- Status: Accepted
- Date: 2026-10-24

## Decision
Accounts and the My Downloads web page are first-class product requirements. Add an API service for identity integration, user profiles/security settings, device/session management, and user-scoped download-history metadata sync.

The identity implementation uses a replaceable standards-based OAuth/OIDC adapter. Do not choose or implement a concrete identity vendor until deployment, pricing, privacy, region, MFA, recovery, and operational requirements have been reviewed. Native clients use Authorization Code + PKCE via the system browser.

## Data policy
Media files remain on users' devices by default. The account stores only minimum necessary history metadata following explicit sync preference. Cloud file storage is separate and requires an explicit product/security/rights decision.

## Security invariants
- Server-derived user identity only.
- Owner authorization enforced on every data access, not only UI filtering.
- Isolation tests must try cross-account reads, writes, deletes, exports, and sync conflicts.
- No secrets or signed URL query strings in logs/history/analytics.
- API and sync outages must not prevent local direct downloads.
- Export, per-record deletion, account deletion, session revocation, and documented retention are required.

## Consequences
A backend, database migrations, rate limiting, monitoring, recovery operations, and privacy policy are required. Multi-user safety becomes a core release gate, not a later feature.
