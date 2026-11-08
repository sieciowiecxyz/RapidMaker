# Revoke the current session family on logout

## Goal
- A successful logout revokes a concurrently rotated successor from the observed session family; independent device sessions remain valid.
- Preserve cookie/public response formats, MFA rules and frontend style. No new dependencies or background worker.

## Scope and context
- Storage auth/credentials/passkeys, API logout, one indexed session-family column.
- Current proof: `security-session-logout-20261108/`; the latest fix reports stale-token logout truthfully but does not revoke its successor.
- Design preview only: `security-session-family-design-20261108/`. It is not compiled or accepted evidence.
- Reuse the auth advisory lock before session rows. Family deletion must execute after lock acquisition under READ COMMITTED.
- A new primary login starts a new family; security/MFA rotations inherit it, including admin bootstrap that deletes the old session.
- No normal-service restart or schema deployment. Tests use disposable PostgreSQL; headless only, without email/payment providers.

## Milestones

### 1. Consistent family identity
- [x] Verified
- Changes: migration and private storage identity; inherit rotations, create independent roots.
- Acceptance: existing tokens remain usable, public JSON omits family ID; bootstrap and password/passkey/MFA tests remain valid.
- Verify: locked app build, existing focused PostgreSQL credential/passkey tests on migrated private databases; formatting/diff checks.

### 2. Atomic family logout
- [x] Verified
- Changes: logout uses observed identity and existing auth lock; retain stale-request failure semantics.
- Acceptance: controlled rotation-first/logout-second gives logout success and successor 401; other family stays authenticated. Logout-first rejects the old rotation.
- Verify: controlled real API/PostgreSQL test, plus ordinary logout/retry and logout-all checks; save safe statuses, hashes and cleanup results.

### 3. Review and concise documentation
- [x] Verified
- Changes: update auth/security contracts and current goal checkpoint.
- Acceptance: Sol xhigh review has no unresolved concrete blocker; migration/legacy limitations are explicit. No claim of 10/10 or production deployment.
- Verify: inspect final diff and frozen source/test hashes; commit verified scope.

## Progress
- Current: milestones 1–3 verified in code and disposable environments; normal service/schema deployment NOT RUN.
- Evidence: `frontend/tests/artifacts/security-session-family-20261108/final-hashes.json`; locked app build, 3 focused PostgreSQL tests, 2 API cookie unit tests and 4 controlled native logout/rotation cases PASS. Existing resend PostgreSQL test was not run.
- Migration 24→25 preserves two existing tokens as independent families. Virtual native-WebAuthn verifies bootstrap role-trigger inheritance and fresh primary roots with required MFA; not a physical authenticator test.
- Sol xhigh reviewed frozen source/result hashes without a concrete blocker. Net production growth: 41 Rust/SQL lines; no new dependency, cookie format or public JSON field.
- Boundary: no historical rotation lineage can be recovered for rows already replaced before migration. A later independent primary login is outside the logged-out family.
