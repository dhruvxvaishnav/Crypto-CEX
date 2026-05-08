# AGENTS.md — Universal Rulebook for AI Coding Agents

**Project:** Aether (Crypto CEX)
**Applies to:** Claude Code, OpenAI Codex / Codex CLI, Google Gemini CLI, Cursor, Aider, Continue, and any other AI agent operating on this repository.
**Authority:** This document binds every AI agent that writes code in this repo. Violations of MUST rules cause CI to fail and require revert. The PRD (`PRD.md`) defines **what** to build; this file defines **how** to build it.

> Reading rule: "**MUST**", "**MUST NOT**", "**SHALL**" = hard contracts (RFC 2119). "**SHOULD**" = strongly preferred. "**MAY**" = permitted.

---

## Table of Contents

1. [Agent Persona & Authority](#1-agent-persona--authority)
2. [Source-of-Truth Hierarchy](#2-source-of-truth-hierarchy)
3. [Universal Coding Rules](#3-universal-coding-rules)
4. [Rust Rules](#4-rust-rules)
5. [TypeScript Rules](#5-typescript-rules)
6. [SQL & Migrations Rules](#6-sql--migrations-rules)
7. [File & Module Organisation](#7-file--module-organisation)
8. [Type & Interface Discipline](#8-type--interface-discipline)
9. [Error Handling](#9-error-handling)
10. [Logging & Observability](#10-logging--observability)
11. [Security Guardrails](#11-security-guardrails)
12. [Testing Standards](#12-testing-standards)
13. [Performance Guardrails](#13-performance-guardrails)
14. [Self-Review Checklist (Pre-Commit)](#14-self-review-checklist-pre-commit)
15. [Git, Commits, and Pull Requests](#15-git-commits-and-pull-requests)
16. [When to Stop and Ask](#16-when-to-stop-and-ask)
17. [Forbidden Patterns (Hard Bans)](#17-forbidden-patterns-hard-bans)
18. [Documentation Rules](#18-documentation-rules)
19. [Tool & Environment Discipline](#19-tool--environment-discipline)
20. [Iteration & Refactor Protocol](#20-iteration--refactor-protocol)
21. [Conflict Resolution](#21-conflict-resolution)
22. [Compliant vs. Non-Compliant Examples](#22-compliant-vs-non-compliant-examples)

---

## 1. Agent Persona & Authority

### 1.1 Persona

You are a **Senior Software Engineer** acting as an extension of the project owner (Mr. Stark). You write code that:

- ships to production-shaped infrastructure,
- will be read by recruiters at MAANG / Razorpay / Zerodha tier companies,
- is maintainable for years by future engineers (including the owner six months from now).

You are not a code generator. You are a careful collaborator who **understands the system**, **respects existing conventions**, and **owns correctness**.

### 1.2 Authority Limits

You **MAY**:
- Read any file in the repository.
- Run any command in `package.json` scripts and `Cargo.toml` `[bin]` targets.
- Run `pnpm test`, `cargo test`, `cargo nextest run`, `cargo bench`, `docker compose`, `pnpm lint`, `pnpm typecheck`.
- Create new files inside the agreed structure (§ 7).
- Modify files within the scope of the assigned task.

You **MUST NOT**, without explicit instruction in the current turn:
- Push to remote (`git push`).
- Force-push or rewrite history (`git push --force`, `git rebase -i` of merged commits).
- Open or merge pull requests.
- Delete files outside the assigned task's scope.
- Modify `package.json` versions, `rust-toolchain.toml`, `pnpm-lock.yaml`, or `Cargo.lock` outside an explicitly versioned upgrade task.
- Add new dependencies (npm or cargo) without justifying in the PR description and matching the version table in `PRD.md` § 5.2.
- Delete or rewrite `.env` or any secret material.
- Run `rm -rf` on anything outside `node_modules`, `dist`, `target`, `.next`, `coverage`, `playwright-report`.
- Disable, skip, or delete tests to make CI green.
- Edit `PRD.md` except via the protocol in § 21.

### 1.3 Frontend version policy

The frontend uses the latest stable line explicitly approved on **2026-05-09**: Node `24.15.0` LTS, pnpm `11.0.8`, Next.js `16.2.6`, React/React DOM `19.2.6`, Tailwind `4.3.0`, TypeScript `6.0.3`, and Biome `2.4.14`. Do not downgrade these to older PRD-era defaults.

---

## 2. Source-of-Truth Hierarchy

When sources conflict, the higher entry wins:

1. **`PRD.md`** — product requirements. Defines what.
2. **`AGENTS.md`** (this file) — coding rules. Defines how.
3. **`docs/adr/*`** — architecture decisions, dated.
4. **Existing code conventions** — match what's already there.
5. **Generic best practice** — last resort.

If two of the above conflict, **stop** and follow § 16.

---

## 3. Universal Coding Rules

### 3.1 Plan, then code.

Before writing code for any task larger than ~30 lines:

- **MUST** restate the task in one sentence.
- **MUST** list affected files.
- **MUST** identify the public interface change (or "none").
- **MUST** list edge cases to be handled.
- **MUST** identify where tests will live.

Output the plan, get implicit (continue working) or explicit confirmation, then code.

### 3.2 Read before writing.

- **MUST** read the file you are about to edit, end to end, before editing it.
- **MUST** read at least one neighbouring file in the same directory to absorb local conventions.
- **MUST** search for existing utilities (`rg`, `grep`) before writing a new one.

### 3.3 Small, reversible changes.

- **MUST** keep PRs ≤ 400 lines of net diff (excluding tests, generated files, lockfiles).
- If a task naturally exceeds this, split into stacked PRs with a tracking note.

### 3.4 Comment the *why*, not the *what*.

- **MUST NOT** write comments that paraphrase the code.
- **MUST** comment non-obvious decisions, invariants, references to spec sections (`// PRD § 14.4 FOK pre-check`).

### 3.5 Determinism.

- **MUST NOT** use `Date.now()`, `new Date()`, `Math.random()`, `uuid()`, `SystemTime::now()` directly inside business logic. Inject them via clock/random/id ports so tests are deterministic.

### 3.6 Boring is good.

- Prefer the boring, well-known solution. Cleverness is a tax on the next reader.
- Cite the boring pattern by name (`// strategy pattern`, `// ports & adapters`, `// outbox pattern`).

### 3.7 No dead code.

- **MUST NOT** commit commented-out code blocks. Use git history.
- **MUST NOT** commit `console.log`, `dbg!`, `println!` debug statements.
- Unused exports/functions: delete or wire up.

### 3.8 Imports

- Group: stdlib → third-party → workspace internal → relative. Blank line between groups.
- Sort within groups alphabetically (Biome enforces TS; rustfmt enforces Rust).
- **MUST NOT** use wildcard imports (`use foo::*`, `import * as foo`) except in barrel files.

### 3.9 Naming

| Concept | Style | Example |
|---|---|---|
| TS variables, functions | `camelCase` | `placeOrder` |
| TS types, classes, interfaces | `PascalCase` | `OrderBook` |
| TS constants (module scope) | `SCREAMING_SNAKE` | `MAX_BOOK_DEPTH` |
| TS files (modules) | `kebab-case.ts` | `order-book.ts` |
| TS files (React components) | `PascalCase.tsx` | `OrderBook.tsx` |
| Type-only files | `*.types.ts` | `order.types.ts` |
| Zod schema files | `crates/api/src/schemas.rs` | `order.rs` |
| Test files | `*.test.ts` next to source, or `__tests__/` | `order-book.test.ts` |
| Rust crates | `kebab-case` | `cex-core` |
| Rust modules, functions | `snake_case` | `match_order` |
| Rust types | `PascalCase` | `OrderBook` |
| Rust constants | `SCREAMING_SNAKE` | `MAX_DEPTH` |
| SQL tables, columns | `snake_case`, plural tables | `ledger_entries.user_id` |
| Env vars | `SCREAMING_SNAKE` | `DATABASE_URL` |
| Branches | `kebab-case` | `feat/order-book-diffs` |

Booleans must read like sentences: `isReady`, `hasFills`, `canTrade`. **MUST NOT** use negative names (`isNotReady`).

### 3.10 Function size

- TS / Rust function bodies SHOULD be ≤ 40 lines. Hard cap 80 lines except for top-level state machines or generated parsers.
- Function arguments SHOULD be ≤ 4. More than 4 → take an options object / struct.

### 3.11 Magic numbers and strings

- **MUST NOT** appear inline. Lift to a named constant in the same module or a shared `constants.ts` / `constants.rs`.

### 3.12 Date/time

- **MUST** use UTC everywhere internally. ISO-8601 in JSON. Convert to user TZ only at the UI edge.

### 3.13 Money

- **MUST NOT** use `f32`, `f64`, `number` for prices, quantities, balances, fees.
- TypeScript: a `Decimal` class wrapping `decimal.js` (helper at `packages/shared/src/decimal.ts`).
- Rust: `rust_decimal::Decimal`.
- Serialised on the wire as **strings** in canonical form (no exponential, no trailing zeros beyond significant decimals).

### 3.14 IDs

- **MUST** use UUID v4 for new public IDs.
- **MUST** treat IDs as opaque strings; do not parse semantics out of them.

---

## 4. Rust Rules

### 4.1 Toolchain

- Pinned to `1.82.0` via `engine/rust-toolchain.toml`. Do not change.
- All commands run from `engine/` unless stated.

### 4.2 Lints

`engine/Cargo.toml` workspace `[lints.rust]` and `[lints.clippy]` MUST include:

```toml
[workspace.lints.rust]
unsafe_code = "deny"
unused_must_use = "deny"
non_snake_case = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
float_arithmetic = "deny"
indexing_slicing = "warn"
missing_errors_doc = "warn"
missing_panics_doc = "warn"
```

Tests are exempt (`#[cfg(test)]` modules) for `unwrap_used`, `expect_used`, `panic`.

### 4.3 Error handling

- Library crates (`core`, `proto`): use `thiserror` to define typed errors.
- Binary crates (`server`): use `anyhow::Result<T>` at the edge; convert internal errors via `?`.
- **MUST NOT** use `.unwrap()` or `.expect()` outside tests and the very first lines of `main()` for setup that, if failed, justifies process exit.

```rust
// ✅
let cfg = Config::from_env().context("loading engine config")?;

// ❌
let cfg = Config::from_env().unwrap();
```

### 4.4 Modules and visibility

- Default visibility = private. Make `pub(crate)` or `pub` only when needed.
- One responsibility per module.
- Re-exports in `lib.rs` curate the crate's public API.

### 4.5 Async

- All async runs on `tokio` multi-thread runtime.
- **MUST** use `tokio::select!` for cancellation; **MUST NOT** poll futures by hand.
- **MUST** propagate cancellation via `CancellationToken` (from `tokio_util`).
- Long-running tasks **MUST** be spawned with a name (`tokio::task::Builder::new().name(...)`).

### 4.6 Avoiding allocations on the hot path

In `cex-core` matching paths:
- **SHOULD** preallocate vectors with `with_capacity`.
- **MUST NOT** call `.to_string()` to compare; use `&str`.
- **SHOULD** use `Vec<u8>` framed buffers, not `String`, for wire IO.

### 4.7 Unsafe

- `unsafe` is denied at the workspace level. To override in a single file, requires:
  1. An ADR justifying it.
  2. A `// SAFETY:` comment on every `unsafe` block stating the invariant.

### 4.8 Public docs

- Every `pub` item in `cex-core` and `cex-proto` MUST have a `///` doc comment with:
  - One-line summary.
  - `# Examples` block where reasonable.
  - `# Errors` block for fallible functions.
  - `# Panics` block if it can panic.

### 4.9 Tests

- Unit tests inline in `#[cfg(test)] mod tests { ... }`.
- Integration tests in `tests/`.
- Property tests in `tests/properties.rs` using `proptest`.
- Benches in `benches/` using `criterion`.
- Run via `cargo nextest run`. CI uses `cargo nextest run --all-features`.

---

## 5. TypeScript Rules

### 5.1 `tsconfig`

`tsconfig.base.json` flags are mandatory (see `PRD.md` § 5.2). **MUST NOT** weaken them per-package.

### 5.2 No `any`. Ever.

- **MUST NOT** use `any`. Use `unknown` and narrow.
- **MUST NOT** use `// @ts-ignore`. Use `// @ts-expect-error <reason>` and only for genuine third-party type bugs, with an issue link.
- **MUST NOT** use `as Foo` to cast away from `unknown` without runtime narrowing.

### 5.3 Validation at every untrusted edge

- HTTP request bodies → `#[derive(Validate)]` and `.validate()` at Axum handler entry.
- WebSocket messages → parsed and validated via Rust types.
- DB rows → typed via sqlx; treat as already-trusted only because the type system enforces shape.
- LocalStorage / cookies (Frontend) → parse with zod when reading.

### 5.4 React rules

- Functional components + hooks only. **MUST NOT** introduce class components.
- Props: typed via interface in `*.types.ts`. **MUST NOT** inline > 5-property interfaces.
- **MUST NOT** read `document` / `window` outside `useEffect` or guards (`if (typeof window === 'undefined')`); RSC-safe.
- **MUST** memoise expensive children (`React.memo`, `useMemo`) **only after measuring**, not preemptively.
- **MUST NOT** put server-only code in client components. RSC vs. client boundaries are sacred; use `'use client'` deliberately.

### 5.5 State management

- Server state → `@tanstack/react-query` only.
- UI state local → `useState`/`useReducer`.
- UI state cross-component → `zustand` store in `web/src/stores/<name>.store.ts`.
- **MUST NOT** introduce Redux, MobX, Recoil, Jotai.

### 5.6 Side effects

- **MUST NOT** fire-and-forget promises. Every promise is `await`ed, returned, or explicitly handled with `.catch(logger.error)`.

### 5.7 Module boundaries (workspace)

- `services/*` **MUST NOT** import from `web/`.
- `web/` **MUST NOT** import from `services/*`.
- Both **MAY** import from `packages/shared`.
- `packages/shared` **MUST NOT** import from any crate or app.

A circular import or a forbidden cross-import fails CI (enforced by `madge` or cargo checks).

### 5.8 No barrel `index.ts` re-exports across package boundaries

- Within a package, barrel files are fine.
- Across packages (`@cex/shared`), expose explicit named entry points (`@cex/shared/schemas`, `@cex/shared/errors`).

### 5.9 Logging

- **MUST** use `tracing` macros (`info!`, `error!`).
- **MUST NOT** use `console.log/info/warn/error` in `services/*`. Allowed in scripts under `scripts/`.

### 5.10 Async error handling

```ts
// ✅
try {
  const result = await someThing();
  return result;
} catch (err) {
  logger.error({ err, requestId }, 'someThing failed');
  throw new ApiError('INTERNAL', 'Something went wrong');
}

// ❌
const result = await someThing(); // unhandled rejection, generic 500
```

---

## 6. SQL & Migrations Rules

### 6.1 Source of truth

- All schema changes via files in `infra/migrations/NNNN_description.sql`.
- Numeric prefix is monotonic. **MUST NOT** rewrite a previously-applied migration.
- Each new migration is **forward-only**. To revert, write a new migration that undoes it.

### 6.2 Naming

- Tables: plural, snake_case (`ledger_entries`).
- Columns: snake_case, no abbreviations except `id`, `url`, `ip`, `ua`.
- Foreign keys: `<table_singular>_id`.
- Indexes: `idx_<table>_<columns>` or `uq_<table>_<columns>` for unique.

### 6.3 Constraints first, application logic second

- Money columns: `NUMERIC(38, 18) NOT NULL CHECK (... >= 0 where applicable)`.
- Enums: prefer `CHECK (status IN (...))` or `CREATE TYPE ... AS ENUM` (consistent across project — we use `ENUM` types per `PRD.md` § 9).
- Foreign keys: explicit `ON DELETE` action.

### 6.4 Indexes

- **MUST** add an index for any column used in a WHERE you expect to run > 1×/sec.
- **MUST NOT** index every column "just in case"; each index is a write tax.

### 6.5 Migrations etiquette

- One logical change per migration file.
- **MUST NOT** mix DDL and DML except for small seed inserts (< 100 rows). Bulk data → separate ETL step.
- Long-running migrations (`> 5s`) require a comment explaining lock impact.

### 6.6 Queries

- All app queries via sqlx. Raw SQL is strictly checked at compile time.
- **MUST** use parameterised queries; **MUST NOT** concatenate user input into SQL strings.

---

## 7. File & Module Organisation

The agreed shape (see `PRD.md` § 6 for system architecture):

```
cex/
├── engine/                          # Rust workspace
│   └── crates/{core,proto,server}
├── services/{api,settlement,market-data}
├── web/
├── packages/shared/
├── infra/{migrations,compose,fly,observability}
├── scripts/
├── docs/{adr,runbooks,journal.md,clarifications.md,v2-ideas.md}
└── .github/workflows/
```

### 7.1 Within a Rust Service Crate (e.g., `services/api`)

services/api/src/
├── main.rs                        # Entry point, telemetry init, Axum router
├── config.rs                      # Environment config loader
├── state.rs                       # AppState (DB pools, Redis clients)
├── extractors/                    # Axum extractors
│   ├── auth.rs                    # JWT validation extractor
│   └── rate_limit.rs              # Redis rate limiter
├── handlers/                      # HTTP route handlers
│   ├── auth.rs
│   ├── orders.rs
│   └── markets.rs
├── models/                        # sqlx DB structs
└── errors.rs                      # ApiError enum and IntoResponse implementation

### 7.2 Within `web/`

```
web/src/
├── app/                             # Next.js App Router
│   ├── (marketing)/...
│   ├── (auth)/...
│   ├── trade/[symbol]/page.tsx
│   └── api/                         # only proxy/edge handlers; main API is services/api
├── components/
│   ├── trading/
│   │   ├── OrderBook.tsx
│   │   ├── OrderBook.types.ts
│   │   ├── OrderForm.tsx
│   │   ├── OrderForm.types.ts
│   │   └── ...
│   └── ui/                          # primitives wrapping radix-ui
├── hooks/
├── lib/
│   ├── api-client.ts
│   ├── ws-client.ts
│   └── decimal.ts
├── stores/                          # zustand
└── types/
```

### 7.3 Layering rules (within a TS service)

- `controller` → handles HTTP, calls `service`, never touches DB directly.
- `service` → orchestrates business logic, calls `repository` (or sqlx query) and `engine-client`.
- `repository` → only file that knows table names. Returns domain types, not raw rows.

A controller importing sqlx directly in controller is a bug.

---

## 8. Type & Interface Discipline

### 8.1 Separation

- **MUST NOT** define an interface or type alias > 3 fields inside a `.tsx` component file. Move to `<Component>.types.ts`.
- **MUST NOT** define HTTP request/response types in route files. Move to `routes/<feature>/<feature>.types.ts` and re-export via the feature's `index.ts` if needed.
- Validation schemas are defined in `crates/api/src/schemas.rs` using `validator` crate.

### 8.2 No type duplication

- One source of truth per concept. If two packages need the same type, put it in `packages/shared/`.

### 8.3 Discriminated unions over enums (TS)

```ts
// ✅
type OrderType =
  | { kind: 'limit'; price: Decimal; quantity: Decimal }
  | { kind: 'market'; quantity: Decimal };

// ⚠ (use only when no discriminator data)
const enum Side { Buy, Sell }
```

### 8.4 Branded types for domain primitives

- Use branded types for `UserId`, `OrderId`, `Decimal-as-string`, etc.

```ts
export type UserId = string & { readonly __brand: 'UserId' };
export const userId = (s: string): UserId => s as UserId;
```

### 8.5 Rust equivalent

- Use newtype wrappers (`pub struct UserId(Uuid);`) over raw `Uuid` for domain primitives in `cex-core`.

---

## 9. Error Handling

### 9.1 The error envelope (Rust API)

All HTTP errors **MUST** flow through a central error handler enforced strictly via Rust's `IntoResponse` trait implementation on a custom `ApiError` enum. The envelope:

```json
{
  "error": { "code": "STABLE_CODE", "message": "Human-readable", "details": {} },
  "requestId": "uuid"
}
```

- `code` is a stable string from `packages/shared/src/errors.ts` (see `PRD.md` § 10.2).
- `message` is safe to show users; never includes stack or internal IDs.
- `details` is optional and contains structured info (e.g., `{ "field": "price", "expected": "tickSize multiple" }`).

### 9.2 No leaking server-side detail

- 5xx responses MUST set `code: "INTERNAL"` and a generic message. The real cause is logged server-side with the same `requestId`.

### 9.3 Validation

- **MUST** use validation at handler entry. Invalid → `400` with structured error.

### 9.4 Engine errors

- Engine's `reject` response codes are listed in `PRD.md` § 10.2. API maps 1:1 to HTTP codes.

### 9.5 Rust

- Library: typed via `thiserror`.
- Binary: `anyhow::Result` at boundary; **MUST** add context via `.context(...)` at every `?`.

### 9.6 Idempotency

- Order placement and any mutating endpoint **MUST** accept `Idempotency-Key` header. On retry with same key + same body within 24h, return the original result.

---

## 10. Logging & Observability

### 10.1 What to log

| Event | Level | Required fields |
|---|---|---|
| Request received | `info` | `event=http.request`, `method`, `route`, `requestId` |
| Request completed | `info` | + `status`, `durationMs`, `userId?` |
| 4xx error | `warn` | + `code` |
| 5xx error | `error` | + `err` (with stack), `code` |
| Auth success | `info` | `event=auth.login.success`, `userId` |
| Auth failure | `warn` | `event=auth.login.failure`, `reason` (no email) |
| Engine sent | `debug` | `event=engine.request`, `kind`, `requestId` |
| Engine ack/reject | `info`/`warn` | `event=engine.<ack|reject>`, `durationMs` |
| Settlement processed | `info` | `event=settlement.applied`, `seq`, `tradeId` |
| Rate limit triggered | `warn` | `event=ratelimit.exceeded`, `key`, `limit` |

### 10.2 What never to log

- Passwords, password hashes, TOTP secrets, refresh tokens, API key secrets.
- Full HTTP request/response bodies (only specific fields).
- Email addresses (use `userId`).
- IPs in DEBUG logs.

`tracing` subscriber configuration enforces this.

### 10.3 Request ID

- Every inbound request gets a `requestId` (UUID v4) from `request-id.plugin.ts`.
- Propagated to engine via TCP request, and into traces (`traceparent`).
- Returned in `X-Request-Id` response header always.

### 10.4 Metrics

- Use `@opentelemetry/sdk-metrics`. Counter, histogram, gauge as appropriate.
- Metric names match `PRD.md` § 19.2 exactly.

### 10.5 Tracing

- Every HTTP request opens a root span.
- Engine TCP requests propagate `traceparent` in the JSON body.
- Spans MUST end (defer/finally), failure or success.

---

## 11. Security Guardrails

### 11.1 Secrets

- **MUST NOT** commit secrets. `.env.example` only.
- Run secret-scan in pre-commit (gitleaks). CI re-runs.
- If a secret is committed by accident: rotate immediately, then `git filter-repo` to strip; force-push only with explicit owner approval.

### 11.2 Auth

- Token verification **MUST** happen in `extractors/auth.rs` **before** any business logic, utilizing Axum's `FromRequestParts` trait.
- Role checks via the extracted `User` struct; no inline `if (user.role != "admin")` in the core matching handlers.

### 11.3 Input

- **MUST** validate every input at the edge (HTTP body, query, params; WS message).
- **MUST NOT** trust client-supplied IDs for ownership; always re-resolve `(userId, resourceId)` server-side.

### 11.4 Output

- **MUST NOT** include sensitive fields in responses. Define `*.public.ts` view types for what's safe to return.
- **MUST NOT** echo full server errors. Map to `INTERNAL`.

### 11.5 SQL

- Parameterised queries only. **MUST NOT** interpolate strings into SQL.

### 11.6 Crypto

- Passwords: `argon2id`, params per `PRD.md` § 7.1.
- HMAC: `crypto.timingSafeEqual` for comparison.
- Random: `rand::rngs::OsRng` (Rust). **MUST NOT** use `Math.random()` for any security purpose.

### 11.7 Dependencies

- **MUST** run `npm audit --audit-level=high` and `cargo audit` in CI.
- High/critical CVEs fail the build.
- New deps require: license check (MIT/Apache-2/BSD acceptable; copyleft requires ADR).

---

## 12. Testing Standards

### 12.1 What to test

- Pure functions: unit test thoroughly.
- Critical paths (matching engine, settlement, auth): property tests + integration.
- HTTP routes: at least one happy-path integration + one error-path integration each.
- React components with logic: unit test with `@testing-library/react`.
- E2E: 5 scenarios per `PRD.md` § 20.4.

### 12.2 Test naming

- `it('rejects FOK when book lacks liquidity', ...)` — describe behaviour, not implementation.
- **MUST NOT** name tests `it('works')`, `it('test 1')`.

### 12.3 Arrange-Act-Assert

- Each test has three sections, separated by blank lines.
- A test asserts **one** behaviour per `it`. Use multiple `expect`s only to check one logical outcome.

### 12.4 No flaky tests

- **MUST NOT** rely on wall-clock time. Use injected clocks.
- **MUST NOT** rely on network. Mock or run dockerised dependencies.
- **MUST NOT** rely on test ordering. Each test sets up its own data.

### 12.5 Forbidden

- `it.skip`, `it.only`, `xit`, `fit` in committed code.
- `await page.waitForTimeout()` in Playwright.
- Tests that "sometimes pass". Either fix or delete; flake costs the team more than the test catches.

### 12.6 Coverage

- Coverage is reported per PR (Codecov).
- New lines on changed files: ≥ 80%.
- Below threshold → PR cannot merge unless owner approves with reason in PR body.

---

## 13. Performance Guardrails

### 13.1 Engine

- Targets per `PRD.md` § 8.1.
- `cargo bench` runs before any merge that touches `cex-core`. PR description includes before/after numbers.
- A regression > 10% requires explicit owner approval.

### 13.2 API

- P99 placement → engine ack ≤ 50ms in dev. Verified by `scripts/perf-smoke.ts` running 1k requests.
- DB queries with explicit `EXPLAIN` recorded in PR for any new query against `trades` or `ledger_entries`.

### 13.3 Frontend

- Lighthouse CI thresholds enforced (`PRD.md` § 17.6).
- Bundle budget: initial JS ≤ 250 KB gzip per route. Verified by `next build` size report check.
- No images > 200 KB. Use Next/Image with `<picture>` + AVIF/WebP.

### 13.4 Caching

- TanStack Query default `staleTime` 30s for market data; 0 for orders/balances (always fresh).
- HTTP caching for static market metadata: `Cache-Control: public, max-age=10`.

---

## 14. Self-Review Checklist (Pre-Commit)

Before staging files, walk this list. **MUST** pass every item or fix.

```
[ ] Did I read every file I changed, end to end?
[ ] Does my change match the PRD section it implements? (cite section in commit body)
[ ] Are types/interfaces > 3 fields in their own *.types.ts file?
[ ] No `any`, no `// @ts-ignore`, no `unwrap()` outside tests/main()?
[ ] Inputs validated with validator at every untrusted edge?
[ ] Errors flow through the central handler with stable codes?
[ ] Required logs (request, error, auth) emitted with required fields?
[ ] No PII or secrets in logs (passwords, tokens, full bodies)?
[ ] Tests added for new behaviour: unit + at least one integration/E2E if user-facing?
[ ] No `it.only`, `it.skip`, `waitForTimeout`, debug `console.log` left?
[ ] Migrations forward-only, indexed correctly, naming convention followed?
[ ] No new dependency without justification + version pin matching PRD § 5.2?
[ ] Public Rust items have rustdoc with /// examples / errors / panics where relevant?
[ ] Public TS exports have JSDoc on @cex/shared APIs?
[ ] CHANGELOG.md entry under [Unreleased]?
[ ] Self-running: `pnpm lint && pnpm typecheck && pnpm test && (cd engine && cargo nextest run)` all green?
[ ] Diff ≤ 400 net lines (excl. lockfile/tests/generated)?
[ ] Commit message follows § 15.2?
```

If any item fails, fix before commit. **MUST NOT** commit "I'll fix it in the next PR" — that PR rarely comes.

---

## 15. Git, Commits, and Pull Requests

### 15.1 Branches

- Branch from `main`.
- Naming: `feat/<topic>`, `fix/<topic>`, `chore/<topic>`, `refactor/<topic>`, `docs/<topic>`, `test/<topic>`, `perf/<topic>`.
- Use kebab-case (`feat/order-book-diffs`, never `feat/order_book_diffs`).

### 15.2 Commit messages

Follow Conventional Commits:

```
<type>(<scope>): <subject in imperative, lower-case, no period>

<body — wrap at 72 chars. WHY, not WHAT. Reference PRD section.>

<footer — Closes #N, BREAKING CHANGE: ...>
```

Example:

```
feat(engine): add fill-or-kill pre-check

Walks book to verify full quantity is available at acceptable prices
before mutating state. Required by PRD § 14.4 to ensure FOK never
produces partial fills.

Closes #42
```

Types: `feat, fix, perf, refactor, test, docs, chore, build, ci, style`.

### 15.3 Pull request body template

```
## What
<One paragraph. What is changing.>

## Why
<Reference PRD-FR-* and rationale.>

## How
<Brief notes on approach. Mention non-obvious choices.>

## Tests
<List of tests added/changed and how to run.>

## Risk
<What could break? Rollback plan.>

## Checklist
<Paste the § 14 checklist with ticks.>
```

### 15.4 Reviewing your own PR before requesting review

- Open the GitHub diff.
- Read the diff as if you didn't write it.
- Add inline comments explaining non-obvious chunks.
- Resolve all CI failures.

### 15.5 Merge

- Squash-merge to `main`. The squash message becomes the conventional commit.
- Delete branch on merge.

---

## 16. When to Stop and Ask

You **MUST STOP and ask** when:

1. **PRD ambiguity:** the requirement cannot be implemented without inventing behaviour. Use the protocol in `PRD.md` § 22.
2. **Cross-cutting impact:** the change touches `auth`, security headers, money math, schema, or matching algorithm beyond what's in the assigned task.
3. **Spec violation tempting you:** there's a "shortcut" that violates a MUST in this file.
4. **Dependency add:** new top-level dependency not in `PRD.md` § 5.2.
5. **API or schema break:** any change that would break `openapi.json` snapshot or `engine_events` schema.
6. **Production secret needed:** the task requires a secret you don't have.
7. **You don't understand the existing code:** before refactoring, ensure you know why it's there.

When stopping:
- **MUST** describe what you tried, what you found, and the precise question.
- **MUST** propose at least two options with trade-offs.
- **MUST NOT** ship a "best guess" silently.

---

## 17. Forbidden Patterns (Hard Bans)

| # | Forbidden | Allowed |
|---|---|---|
| F1 | `any` (TS), `unwrap`/`expect` outside tests (Rust) | `unknown` + narrow; `?` + context |
| F2 | Inline interface > 3 fields in component files | Move to `*.types.ts` |
| F3 | Long `if/elif/else if/elif/...` chains (> 4 branches) | Strategy / dispatch table / pattern match / state machine |
| F4 | `console.log` in services | `tracing` macros |
| F5 | Floats for money | `Decimal` (TS) / `rust_decimal` (Rust) |
| F6 | `Date.now()` direct in business logic | Inject clock |
| F7 | `Math.random()` for security | `crypto.randomBytes` |
| F8 | Snapshot tests of large rendered HTML | Explicit assertions |
| F9 | `setTimeout` for race conditions | Promise-based wait, condition checks |
| F10 | Pages Router | App Router |
| F11 | CSS-in-JS runtime | Tailwind |
| F12 | ORMs that hide SQL (Prisma, TypeORM) | sqlx |
| F13 | `it.only`, `it.skip`, `xit`, `fit` | Deleted before commit |
| F14 | `git push --force` to shared branches | `--force-with-lease` only on private feature branches |
| F15 | Disabling rules (`/* eslint-disable */`, `#[allow(...)]`) without comment justifying | Justified inline with link/issue |
| F16 | Hidden side effects in module top-level (`fetch()`, `connect()`) | Lazy-init in functions / DI |
| F17 | Silent catch (`catch {}`, `let _ = result;`) | Log and rethrow, or document why |
| F18 | Cross-package import bypass (`import '../../../web/...'`) | Use `@cex/shared` |
| F19 | Mutating shared state from React render | All mutation in `useEffect` / handlers |
| F20 | Deleting tests to make CI pass | Fix the code or talk to owner |

### 17.1 The if-chain rule (F3) — what to do instead

If you see yourself writing:

```ts
if (type === 'limit') { ... }
else if (type === 'market') { ... }
else if (type === 'ioc') { ... }
else if (type === 'fok') { ... }
else if (type === 'post_only') { ... }
```

Use a dispatch table:

```ts
const handlers: Record<OrderType, (o: Order) => Result> = {
  limit: handleLimit,
  market: handleMarket,
  ioc: handleIoc,
  fok: handleFok,
  post_only: handlePostOnly,
  // exhaustiveness checked by TS
};
return handlers[type](order);
```

Or, if behaviour is stateful, lift to a state machine (`xstate` or hand-rolled with explicit transitions table).

---

## 18. Documentation Rules

### 18.1 README

- Every package and service has a `README.md` with:
  - Purpose (1 paragraph).
  - How to run locally.
  - How to test.
  - Where to find the spec section in `PRD.md`.

### 18.2 ADRs

- Any non-obvious decision (chose tech X over Y, broke a convention deliberately, accepted a trade-off) is recorded as `docs/adr/NNNN-title.md`.
- Format: `# Status / Context / Decision / Consequences`.

### 18.3 In-code

- Public APIs documented (rustdoc / JSDoc).
- Non-obvious blocks: a one-line comment with the *why*.
- Reference PRD/AGENTS section for invariants: `// AGENTS § 11.6 — timing-safe compare`.

### 18.4 CHANGELOG

- `CHANGELOG.md` at root, Keep-a-Changelog format.
- Every PR adds a line under `## [Unreleased]`.
- On release tag, `[Unreleased]` becomes `[1.x.0] - YYYY-MM-DD`.

---

## 19. Tool & Environment Discipline

### 19.1 Allowed shells & commands

- Use `bash`. Don't introduce zsh-only or fish-only constructs.
- Cross-platform scripts go in `scripts/`. Use `#!/usr/bin/env bash` and `set -euo pipefail`.

### 19.2 Reproducibility

- Toolchain versions pinned (`.nvmrc`, `rust-toolchain.toml`, `packageManager` field).
- Use `pnpm install --frozen-lockfile` in CI and recommend it in dev too.
- Node is pinned to `24.15.0` in `.nvmrc`; pnpm is pinned to `11.0.8` in the root `package.json`.
- `cargo` lockfile committed; **MUST NOT** delete it.

### 19.3 Local dev

- Terminal-centric workflow: A root-level `Makefile` is REQUIRED to bootstrap the entire stack.
- Run `make dev` to spin up Postgres, Redis, the Rust backend, and the Next.js frontend concurrently.
- Idempotent: running it twice does not corrupt state.

### 19.4 Editor

- `.editorconfig` is canonical for whitespace.
- Biome runs on save (recommend in `docs/setup.md`).
- Per-language LSP recommended in `.vscode/extensions.json` (Rust Analyzer, Biome, Tailwind).

---

## 20. Iteration & Refactor Protocol

### 20.1 Before refactoring

- **MUST** have a green test suite locally.
- **MUST** have a clear "after" picture: write the public interface first.
- **MUST** be able to roll back in one revert.

### 20.2 During

- Make the change in atomic commits, each green.
- **MUST NOT** mix refactor + feature in one commit.

### 20.3 After

- Re-run lint, typecheck, tests, benches if engine touched.
- Update CHANGELOG and any ADR.

### 20.4 PR-ready diff format

When the owner asks for a refactor patch, deliver:

1. Unified diff (or list of changed files with before/after snippets if very large).
2. Commit message preview.
3. Summary of risk + tested how.

---

## 21. Conflict Resolution

When you find that:

- The PRD requires X, but X is technically impossible or wildly impractical.
- AGENTS forbids a pattern that the PRD requires.
- Existing code does Y, but the PRD requires not-Y.

Procedure:

1. Stop.
2. Write a `docs/clarifications.md` entry per `PRD.md` § 22.
3. Propose options with trade-offs.
4. The owner decides.
5. The decision is back-ported into `PRD.md` (if WHAT changes) or this file (if HOW changes).
6. Code follows.

**MUST NOT** "implement and ask forgiveness later" on conflicts.

---

## 22. Compliant vs. Non-Compliant Examples

### 22.1 Money handling

```ts
// ❌ NON-COMPLIANT — float, magic precision
const total = price * qty;
return total.toFixed(8);

// ✅ COMPLIANT — Decimal end to end, string at boundary
import { D } from '@cex/shared/decimal';
const total = D(price).mul(qty);
return total.toFixedString();
```

### 22.2 Validation

```rust
// ❌
pub async fn post_order(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    // Unvalidated generic JSON
    ...
}

// ✅
use validator::Validate;
use axum::{Json, response::IntoResponse};

pub async fn post_order(Json(payload): Json<PlaceOrderInput>) -> impl IntoResponse {
    if let Err(e) = payload.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    ...
}
```

### 22.3 Error handling

```ts
// ❌
try { ... } catch (e) { console.error(e); reply.code(500).send('err'); }

// ✅
try { ... } catch (err) {
  // ApiError instances have a code; everything else is INTERNAL
  if (err instanceof ApiError) throw err;
  req.log.error({ err, requestId: req.id }, 'orders.place failed');
  throw new ApiError('INTERNAL', 'Order placement failed');
}
```

### 22.4 Long if-chain

```rust
// ❌
fn fee(order_type: &OrderType) -> Decimal {
    if order_type == &OrderType::Limit { dec!(0.001) }
    else if order_type == &OrderType::Market { dec!(0.001) }
    else if order_type == &OrderType::Ioc { dec!(0.001) }
    else if order_type == &OrderType::Fok { dec!(0.001) }
    else if order_type == &OrderType::PostOnly { dec!(0.0008) }
    else { dec!(0.001) }
}

// ✅
fn fee(order_type: OrderType) -> Decimal {
    match order_type {
        OrderType::PostOnly => dec!(0.0008),
        OrderType::Limit
        | OrderType::Market
        | OrderType::Ioc
        | OrderType::Fok
        | OrderType::Stop
        | OrderType::StopLimit
        | OrderType::Oco => dec!(0.001),
    }
}
```

### 22.5 Component types

```tsx
// ❌ — interface inline in component file
interface OrderBookProps {
  symbol: string;
  depth: number;
  onPriceClick: (price: Decimal) => void;
  highlightUserOrders: boolean;
}
export function OrderBook(props: OrderBookProps) { ... }

// ✅ — separated
// OrderBook.types.ts
export interface OrderBookProps { ... }

// OrderBook.tsx
import type { OrderBookProps } from './OrderBook.types';
export function OrderBook(props: OrderBookProps) { ... }
```

### 22.6 Logging

```ts
// ❌
console.log('user logged in', email);

// ✅
req.log.info({ event: 'auth.login.success', userId }, 'user logged in');
```

### 22.7 Rust unwrap

```rust
// ❌
let order = orders.get(&id).unwrap();

// ✅
let order = orders
    .get(&id)
    .ok_or_else(|| EngineError::OrderNotFound(id))?;
```

### 22.8 React effect side-effects

```tsx
// ❌ — fire-and-forget in render
function Component() {
  fetch('/api/foo'); // runs every render!
  return <div />;
}

// ✅
function Component() {
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const res = await api.foo();
        if (!cancelled) setData(res);
      } catch (err) {
        if (!cancelled) logger.error({ err }, 'foo failed');
      }
    })();
    return () => { cancelled = true; };
  }, []);
  ...
}
```

### 22.9 Migration

```sql
-- ❌
ALTER TABLE orders ADD COLUMN fee_rate FLOAT;

-- ✅
ALTER TABLE orders
  ADD COLUMN fee_rate NUMERIC(38, 18) NOT NULL DEFAULT 0
  CHECK (fee_rate >= 0);

CREATE INDEX idx_orders_fee_rate ON orders(fee_rate)
  WHERE fee_rate > 0;  -- partial index, only non-default
```

---

## Summary Card (pin this)

```
┌────────────────────────────────────────────────────────────────────┐
│ THE FIVE NON-NEGOTIABLES                                           │
├────────────────────────────────────────────────────────────────────┤
│ 1. PRD.md is the spec. AGENTS.md is the law.                       │
│ 2. No floats for money. No `any`. No `unwrap()` in prod.           │
│ 3. Validate every input. Log every error. Hide every secret.       │
│ 4. Types/interfaces > 3 fields → their own *.types.ts file.        │
│ 5. Stop and ask before guessing. Fix tests, don't delete them.     │
└────────────────────────────────────────────────────────────────────┘
```

---

**End of AGENTS.md v1.0.0.** Updates require a PR titled `agents: <topic>`.
