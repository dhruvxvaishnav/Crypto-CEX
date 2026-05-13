# Clarifications

## 2026-05-12 — API key secret storage vs HMAC requirement

**Context:** `services/api/src/handlers/api_keys.rs`, `services/api/src/extractors/hmac.rs`

**Question:** PRD §7.5 FR-API-01 says "Secret stored as `argon2id(secret)`", yet PRD §18.4
requires the server to compute `HMAC-SHA256` using the API key secret. These are mutually
exclusive: `argon2id` is one-way so a stored hash cannot be used to compute HMAC.

**Options:**

A. Store the secret in plaintext. No key management overhead, but exposed if DB is leaked.

B. Store the secret reversibly encrypted using `pgcrypto pgp_sym_encrypt` and `PGCRYPTO_KEY`.
   The secret is recoverable for HMAC but is at-rest protected by the symmetric key.

**Decision:** B — `pgp_sym_encrypt`. This matches how TOTP secrets are stored (`totp_secret_encrypted BYTEA`),
is consistent with the project's encryption pattern, and provides meaningful at-rest protection.
The `argon2id` reference in the PRD draft was an error — argon2id is for passwords verified by
comparison, not for keys that must be retrieved. PRD §7.5 FR-API-01 should be updated to read
"Secret stored encrypted with `pgp_sym_encrypt` using `PGCRYPTO_KEY`".

---

## 2026-05-12 — TOTP backup codes storage deferred to v1.1

**Context:** `services/api/src/handlers/totp.rs`

**Question:** PRD §7.1 FR-AUTH-03 AC2 says "returns 8 backup codes hashed with argon2id". The
current implementation generates and returns 8 codes but does not persist them — no
`totp_backup_codes` table was added to the initial schema.

**Decision:** Deferred to a follow-up migration. The handler returns backup codes once (user must
store them), but backup-code login is not yet validated. A `totp_backup_codes` table and fallback
MFA path are tracked in `docs/v2-ideas.md`.

---

## 2026-05-13 — @hookform/resolvers v5 requires zod/v4/core subpath (Turbopack incompatible)

**Context:** `web/src/app/(auth)/*.page.tsx`, `web/src/lib/zod-resolver.ts`

**Question:** `@hookform/resolvers@5.x` imports `"zod/v4/core"` (a zod v4 subpath export). Turbopack
(used by Next.js 16 as the default bundler) cannot resolve subpath exports from the package.json
`exports` map for this particular package, resulting in a build-time module-not-found error.

**Options:**

A. Downgrade to `@hookform/resolvers@3.x` and import from `zod/v3` compat layer. Works but
   requires a workaround for zod v4 API differences.

B. Write an inline `zodResolver` utility that calls `schema.safeParse` directly and converts
   zod v4 issues to react-hook-form `FieldErrors`. Zero external dependency, fully typed.

**Decision:** B — inline `zodResolver` in `web/src/lib/zod-resolver.ts`. This removes
the `@hookform/resolvers` dependency entirely, has no runtime overhead, and avoids bundler
incompatibility. The `@hookform/resolvers` package is kept in `package.json` for potential future
use when Turbopack matures. Tracked as a known Turbopack limitation.

---

## 2026-05-12 — OpenAPI handler annotation completeness

**Context:** `services/api/src/openapi.rs`

**Question:** PRD §10.5 requires all paths generated via `utoipa`. Most existing handlers lack
`#[utoipa::path]` annotations, so only schema stubs and tags are in the current spec.

**Decision:** Full handler annotation deferred to Day 8 polish. The `GET /openapi.json` endpoint,
utoipa dependency, and schema stubs are in place. Annotations are additive with no breaking change.
