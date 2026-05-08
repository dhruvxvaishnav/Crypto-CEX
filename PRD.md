# Product Requirements Document — `Project Aether`

**Version:** 1.0.0 (frozen scope for v1.0)
**Status:** Approved for build
**Owner:** Mr. Stark
**Timeline:** 14 calendar days from kickoff
**Document type:** Single source of truth. If anything in code or in another document contradicts this PRD, this PRD wins. Any deviation requires updating this file first.

> **Reading rule for AI agents and humans:** Treat every "**MUST**", "**MUST NOT**", "**SHALL**" as a hard contract (RFC 2119). "**SHOULD**" is strongly preferred. "**MAY**" is optional. If a requirement is ambiguous, do not guess — open the section labelled "Clarification Protocol" (§ 22) and follow it.

---

## Table of Contents

1. [Vision & Positioning](#1-vision--positioning)
2. [Goals & Non-Goals](#2-goals--non-goals)
3. [Target Users & Personas](#3-target-users--personas)
4. [Scope: Real vs. Simulated](#4-scope-real-vs-simulated)
5. [Tech Stack (Locked)](#5-tech-stack-locked)
6. [System Architecture](#6-system-architecture)
7. [Functional Requirements](#7-functional-requirements)
8. [Non-Functional Requirements](#8-non-functional-requirements)
9. [Data Model](#9-data-model)
10. [REST API Contract](#10-rest-api-contract)
11. [WebSocket API Contract](#11-websocket-api-contract)
12. [Engine ↔ API Wire Protocol](#12-engine--api-wire-protocol)
13. [Order Lifecycle & State Machine](#13-order-lifecycle--state-machine)
14. [Matching Engine Specification](#14-matching-engine-specification)
15. [Settlement & Ledger Rules](#15-settlement--ledger-rules)
16. [Market-Maker Bot Specification](#16-market-maker-bot-specification)
17. [Frontend Specification](#17-frontend-specification)
18. [Security Model](#18-security-model)
19. [Observability & Operations](#19-observability--operations)
20. [Testing Strategy](#20-testing-strategy)
21. [14-Day Delivery Plan](#21-14-day-delivery-plan)
22. [Clarification Protocol](#22-clarification-protocol)
23. [Acceptance Criteria & Definition of Done](#23-acceptance-criteria--definition-of-done)
24. [Risks & Mitigations](#24-risks--mitigations)
25. [Glossary](#25-glossary)

---

## 1. Vision & Positioning

### 1.1 Product Vision

Project Aether is a **production-architecture spot crypto exchange** with feature parity to the trading core of Binance/Backpack, intended as a portfolio-defining demonstration of distributed systems engineering. It is **not** a real exchange and never custodies real funds.

### 1.2 Why this exists

To stand out in MAANG and tier-one Indian product company hiring loops by demonstrating, in one repository:

- A **correct, performant matching engine** in Rust with measurable benchmarks.
- **Production patterns**: event sourcing-lite, double-entry ledger, idempotency, CQRS-light read models, snapshot+diff WebSocket protocol, rate limiting, observability.
- **Real-world UX**: a polished trading interface modelled on Backpack Exchange, with TradingView-grade charts, sub-150ms order book updates, and full keyboard control.
- **Operational maturity**: CI/CD, structured logs, distributed traces, dashboards, runbooks.

### 1.3 Differentiators (the "super duper cool" surface)

These are non-negotiable; they are what move the project from "competent" to "memorable":

1. **Eight order types** — limit, market, IOC, FOK, post-only, stop-limit, stop-market, OCO (One-Cancels-Other), iceberg.
2. **Self-trade prevention** with three modes (decrement-and-cancel, cancel-maker, cancel-taker).
3. **Snapshot-plus-diff WebSocket** with monotonic sequence numbers and gap recovery — exactly how Binance and Coinbase publish books.
4. **Public signed REST API** — HMAC-SHA256 signed requests, Binance-compatible signature scheme, so users can write trading bots against Aether.
5. **Proof-of-Reserves page** — Merkle tree commitment of all user balances published nightly; users can verify their inclusion.
6. **Replay mode** — rewind any market to any point in the last 24h and watch trades play forward at 1×/4×/16×.
7. **Real-time portfolio P&L** with FIFO cost basis.
8. **Hotkey trading** — Bloomberg-terminal-style keyboard control (`b`/`s` to side, `1-9` for size %, `Enter` to submit).
9. **Live admin console** with circuit breakers (halt market, freeze user, cancel-all).
10. **Matching engine throughput target ≥ 500,000 orders/sec** single-threaded on commodity hardware (M-series Mac, Ryzen 7, etc.) with a published `criterion` benchmark in the README.

---

## 2. Goals & Non-Goals

### 2.1 Goals (in priority order)

| # | Goal | Measurable success |
|---|---|---|
| G1 | Correct matching engine | 100% pass on conformance suite (§ 14.7); zero invariant violations in 10M-order property fuzz |
| G2 | End-to-end working flows | All 14 user stories in § 7 demonstrably pass E2E |
| G3 | Production-shaped code | Passes self-review checklist in `AGENTS.md` § 14 with no exceptions |
| G4 | Observable | Every request has trace ID; every error has correlation ID; metrics dashboard ships in repo |
| G5 | Recruiter-readable repo | README opens with 90-second video, architecture diagram, and benchmark numbers within first scroll |
| G6 | Deployable on free tier | `make deploy` succeeds against Vercel + Fly.io + Neon + Upstash with $0 spend |

### 2.2 Non-Goals

| # | Non-goal | Rationale |
|---|---|---|
| N1 | Real custody of crypto | Requires HSMs, audits, legal — out of scope |
| N2 | Real KYC/AML providers | Mocked; UI flow only |
| N3 | Fiat on/off ramp | Out of scope |
| N4 | Derivatives (futures, options, perpetuals, margin, lending) | UI-stubbed only with "Coming soon"; matching engine remains spot-only |
| N5 | Mobile native apps | Responsive web only |
| N6 | Multi-region active-active | Single region; horizontal scaling design documented but not implemented |
| N7 | Internationalisation beyond English | English UI; copy externalised so i18n is a future PR |
| N8 | Social/copy trading, P2P, launchpad, NFT, staking | Out of scope, not even stubbed |

---

## 3. Target Users & Personas

### 3.1 Persona A — "Pro Trader Priya"

- Active spot trader, technical, expects keyboard hotkeys, sub-200ms book updates, advanced order types.
- Will judge the product on **order book responsiveness, chart quality, and order-type breadth**.

### 3.2 Persona B — "Bot Builder Bharat"

- Builds trading bots, expects HMAC-signed REST + WS user data stream, rate-limit headers, idempotency keys.
- Will judge on **API ergonomics and stability**.

### 3.3 Persona C — "Curious Recruiter Riya"

- Hiring manager scanning the repo. Has 90 seconds.
- Will judge on **README clarity, demo video, benchmark numbers, code quality on first file opened**.

### 3.4 Persona D — "Admin Operator"

- The operator (you) running the exchange.
- Will judge on **operability**: can I see what's broken, halt a market, freeze an abuser?

Every feature in § 7 maps to at least one persona.

---

## 4. Scope: Real vs. Simulated

This table is binding. The README must reproduce it verbatim.

| Component | Status | Notes |
|---|---|---|
| Matching engine | **Real** | Rust, in-process, deterministic |
| Order book / fills / trades | **Real** | Persisted to Postgres |
| User accounts, auth, 2FA, sessions | **Real** | Argon2id, TOTP, JWT with rotation |
| WebSocket market data | **Real** | Snapshot+diff with seq numbers |
| REST + WS public API | **Real** | HMAC-signed, rate-limited |
| Spot fees, ledger, balances | **Real** | Double-entry, NUMERIC(38,18) |
| Market data feed | **Real** | Binance public WS (no key) for top 50 USDT pairs |
| Order book liquidity | **Real matching, simulated counterparties** | Market-maker bot seeds depth around real Binance mid-price |
| Deposits | **Simulated** | "Faucet" button credits balance after 30s |
| Withdrawals | **Simulated** | Address regex-validated; transitions to `completed` after 30s |
| Proof-of-Reserves | **Real algorithm, simulated reserves** | Real Merkle commitment of real DB balances |
| Futures / margin / options / lending | **Stubbed** | UI shows "Coming soon" |
| KYC / fiat | **Stubbed** | UI flow only |

The phrase "real exchange" is **forbidden** anywhere in the product and README. The phrase "portfolio demonstration" or "demo exchange" must appear in the footer of every page.

---

## 5. Tech Stack (Locked)

Versions are pinned. AI agents **MUST NOT** upgrade them without an explicit approval logged in `CHANGELOG.md`.

Frontend versions were intentionally refreshed to the latest stable line on **2026-05-09** by owner request. Future upgrades follow the same explicit-approval rule.

### 5.1 Languages & Runtimes

| Component | Language | Version | Pinned via |
|---|---|---|---|
| Matching engine | Rust | `1.82.0` | `engine/rust-toolchain.toml` |
| API gateway | Rust | `1.82.0` | `rust-toolchain.toml` |
| Settlement worker | Rust | `1.82.0` | `rust-toolchain.toml` |
| Market-data worker | Rust | `1.82.0` | `rust-toolchain.toml` |
| Frontend | TypeScript on Next.js `16.2.6` | Node `24.15.0` LTS | `.nvmrc`, `package.json` |
| Database | PostgreSQL | `16.4` | `infra/compose/docker-compose.yml` |
| Cache / pub-sub | Redis | `7.4` | same |

### 5.2 Frameworks & Key Libraries

| Concern | Library | Version | Justification |
|---|---|---|---|
| Rust async runtime | `tokio` | `1.42` | Industry standard |
| Rust decimals | `rust_decimal` | `1.36` | Floats are forbidden for money |
| Rust serde | `serde`, `serde_json` | `1.0` | — |
| Rust testing | `proptest`, `criterion`, `cargo-nextest` | `1.5`, `0.5`, latest | Property tests + benchmarks + fast runner |
| HTTP server | `axum` | latest | High-performance Rust HTTP framework |
| Validation | `validator` | latest | Rust struct validation |
| ORM/SQL | `sqlx` | `0.8` | Compile-time checked raw SQL |
| Postgres client | `pg` | `8.x` | — |
| Auth | `argon2`, `jsonwebtoken`, `otplib` | latest stable | argon2id is current best practice |
| Logging | `tracing` + `tracing-subscriber` | latest | JSON structured |
| Tracing | `@opentelemetry/*`, `opentelemetry` (Rust) | latest | OTLP/HTTP exporter |
| Package manager | `pnpm` | `11.0.8` | Workspace package manager |
| Frontend framework | Next.js | `16.2.6` (App Router) | Latest stable verified on 2026-05-09 |
| React | React / React DOM | `19.2.6` | Latest stable verified on 2026-05-09 |
| State | `zustand` + `@tanstack/react-query` | latest | Zustand for ephemeral UI, TanStack Query for server state |
| Forms | `react-hook-form` + `zod` | latest | — |
| Charts | `lightweight-charts` (TradingView) | `4.2` | Free, professional |
| Styling | `tailwindcss`, `@tailwindcss/postcss` | `4.3.0` | Latest stable verified on 2026-05-09 |
| Component primitives | `radix-ui` | latest | Accessible primitives only; we style them |
| Tests (TS) | `vitest`, `@testing-library/react`, `playwright` | `vitest@4.1.5`, Playwright `1.59.1`, latest compatible Testing Library | — |
| TypeScript | `typescript` | `6.0.3` | Latest stable verified on 2026-05-09 |
| Lint+format | `@biomejs/biome` | `2.4.14` | Single binary replaces ESLint+Prettier |
| Pre-commit | `husky` + `lint-staged` | latest | — |

Any deviation requires a `CHANGELOG.md` entry under `## [Unreleased] - Tooling`.

### 5.3 Forbidden Choices

- **Floats for money** (TS or Rust) — use `rust_decimal::Decimal` and a TS string-based decimal helper that wraps `decimal.js`.
- **`any` in TypeScript** — use `unknown` and narrow.
- **`unwrap()` / `expect()` in production Rust paths** — only in tests and `main()` setup.
- **CSS-in-JS runtime libraries** (styled-components, emotion) — use Tailwind only.
- **Next.js Pages Router** — App Router only.
- **ORMs that hide SQL** (Prisma, TypeORM, Diesel) — sqlx only.
- **`console.log`** anywhere in services or workers — use the configured `tracing` logger.

---

## 6. System Architecture

### 6.1 Process Topology

```
                                    Internet
                                       │
                   ┌───────────────────┼───────────────────┐
                   ▼                   ▼                   ▼
         ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐
         │  Next.js (Web)   │  │   Next.js Edge   │  │   Public     │
         │  Vercel          │  │   middleware     │  │   API users  │
         │  - SSR/RSC       │  │   - rate limit   │  │   (bots)     │
         │  - Trading UI    │  │   - geo-block    │  │              │
         └────────┬─────────┘  └────────┬─────────┘  └──────┬───────┘
                  │                     │                   │
                  └────────────┬────────┴───────────────────┘
                               │ HTTPS / WSS
                               ▼
                  ┌─────────────────────────────┐
                  │  API Gateway (Rust, Axum)  │  Fly.io machine #1
                  │  - JWT verify               │
                  │  - HMAC verify (bot API)    │
                  │  - Request validation (validator)   │
                  │  - Rate limit (Redis)       │
                  │  - WS hub (subscriptions)   │
                  │  - Engine client            │
                  └────────┬────────────┬───────┘
                           │            │
              JSON over    │            │  Postgres LISTEN/NOTIFY
              TCP framed   │            │  + reads
                           ▼            ▼
              ┌─────────────────┐  ┌──────────────────┐
              │  Matching Engine│  │  Postgres (Neon) │
              │  Rust binary    │  │  - users         │
              │  Fly.io #1 same │  │  - orders        │
              │  - per-symbol   │  │  - trades        │
              │    actor        │  │  - balances      │
              │  - in-memory    │  │  - ledger        │
              │  - WAL to disk  │  │                  │
              └─────────────────┘  └──────────────────┘
                           ▲                ▲
                           │                │
              ┌────────────┴────────┐  ┌────┴───────────────────┐
              │ Settlement Worker   │  │ Market-Data Worker     │
              │ Fly.io machine #2   │  │ Fly.io machine #2      │
              │ - consume engine    │  │ - Binance WS proxy     │
              │   events            │  │ - market-maker bot     │
              │ - update balances   │  │ - kline cache          │
              │ - write ledger      │  │                        │
              └─────────────────────┘  └────────────────────────┘

              External:
                  - Binance public WS  ← market-data worker
                  - Upstash Redis      ← API rate limit, WS pubsub fan-out
                  - Grafana Cloud      ← logs + metrics + traces
```

### 6.2 Process Responsibilities

| Process | Binary | Responsibilities | Persistence |
|---|---|---|---|
| `web` | Next.js | Trading UI, SSR, edge middleware | None (stateless) |
| `api` | Rust | All HTTP + WS, request validation, auth, rate limit, engine client | None (reads/writes DB) |
| `engine` | Rust | Match orders, maintain books, emit events | In-memory + WAL append-log to disk for crash recovery |
| `settlement` | Rust | Apply engine events to DB (balances, ledger, trades) | Writes Postgres |
| `market-data` | Rust | Binance ingest, market-maker bot, kline cache | Reads/writes Postgres |

### 6.3 Trust Boundaries

- **Frontend ↔ API**: untrusted; all input validated server-side via Axum extractors (§ 10).
- **API ↔ Engine**: trusted; same machine; localhost TCP. Engine refuses non-loopback connections.
- **API ↔ DB**: trusted; least-privilege role per service.
- **Public API ↔ API**: untrusted; HMAC verification + replay protection (§ 18.4).

### 6.4 Failure Domains & Recovery

| Component | Failure mode | Recovery |
|---|---|---|
| Engine crash | Lost in-memory book | Replay WAL on boot; resubscribe DB to backfill (§ 14.6) |
| API crash | Open WS connections drop | Clients reconnect; resume via `lastSeq` parameter (§ 11.5) |
| Settlement crash | Unprocessed events backlog | Resume from last consumed `eventSeq` stored in DB |
| DB outage | Engine continues matching, settlement queues | Backpressure: API rejects new orders if settlement lag > 1000 events |

---

## 7. Functional Requirements

Each requirement has: ID, user story, acceptance criteria. Every AC must be a passing test before the requirement is "done".

### 7.1 Authentication & Account

#### FR-AUTH-01: Email + password signup

- **Story:** As Priya, I create an account with email and password so I can trade.
- **AC:**
  - **AC1:** Email is validated as RFC 5322 minus comments/quoted-strings. Reject otherwise with `400 INVALID_EMAIL`.
  - **AC2:** Password requires ≥ 12 chars, ≥ 1 lower, ≥ 1 upper, ≥ 1 digit, ≥ 1 symbol. Reject with `400 WEAK_PASSWORD` and a list of missing requirements.
  - **AC3:** Password is hashed with `argon2id`, params `memory=64MiB, iterations=3, parallelism=4`. Hash is the only stored form.
  - **AC4:** Duplicate email returns `409 EMAIL_TAKEN` with no user enumeration timing leak (constant-time response, ≥ 500ms).
  - **AC5:** On success: returns access JWT (15 min) + refresh token (30 days), `201 Created`.
  - **AC6:** Account starts with `kyc_level=0`, `status=active`, balances seeded with `0` for every active asset.

#### FR-AUTH-02: Login

- **Story:** As Priya, I log in with my credentials.
- **AC:**
  - **AC1:** Wrong credentials return `401 INVALID_CREDENTIALS` with no distinction between unknown email and wrong password.
  - **AC2:** After 5 consecutive failures within 15 min on the same email **or** same IP, return `429 TOO_MANY_ATTEMPTS` for 15 min.
  - **AC3:** If `totp_enabled=true`, response is `200` with `{ "mfaRequired": true, "mfaToken": "..." }` and **no** access token. Client must call `/auth/mfa` next.
  - **AC4:** Successful login emits a `tracing` log line at `info` with `event=auth.login.success`, `user_id`, `ip`, `ua`, no PII beyond `user_id`.

#### FR-AUTH-03: TOTP 2FA enrolment

- **AC:**
  - **AC1:** `POST /auth/2fa/setup` returns `{ secret, otpauthUri, qr (data-URI PNG) }`. Secret stored encrypted (`pgcrypto pgp_sym_encrypt`).
  - **AC2:** `POST /auth/2fa/verify { code }` enables 2FA after correct 6-digit code; returns 8 backup codes hashed with argon2id.
  - **AC3:** Disabling 2FA requires current TOTP code AND password.

#### FR-AUTH-04: JWT access + refresh rotation

- **AC:**
  - **AC1:** Access JWT is HS256, 15 min TTL, claims: `{ sub: userId, sid: sessionId, kyc, iat, exp, jti }`.
  - **AC2:** Refresh tokens are opaque (32 bytes random base64url), stored hashed (sha256), 30 day TTL.
  - **AC3:** `POST /auth/refresh` rotates both tokens; old refresh token's `revoked_at` is set. Reuse of a revoked refresh token revokes the entire session family and forces re-login (token reuse detection).
  - **AC4:** `POST /auth/logout` revokes the current session.

#### FR-AUTH-05: Email verification (mocked)

- **AC:** Verification link logged to console in dev; clicking sets `email_verified=true`. Trading is allowed without verification but withdrawals require it.

### 7.2 Wallet (Simulated)

#### FR-WALLET-01: Faucet deposit

- **Story:** As Priya I want to test trading, so I credit my account from the faucet.
- **AC:**
  - **AC1:** `POST /wallet/faucet { asset, amount }` is allowed up to 10× per asset per user, max amounts per asset specified in `assets.faucet_max` column.
  - **AC2:** Inserts `deposits` row with `status=pending`, then transitions to `completed` after 30s and credits balance via the ledger (§ 15).
  - **AC3:** Faucet is **disabled** in production deployment if env `FAUCET_ENABLED=false`; for portfolio demo it is `true` with a clear UI badge "Demo faucet — testnet credits".

#### FR-WALLET-02: Withdraw (simulated)

- **AC:**
  - **AC1:** `POST /wallet/withdraw { asset, amount, address }`.
  - **AC2:** Address validated by per-asset regex defined in seed data (BTC, ETH, SOL, etc.).
  - **AC3:** Requires email verification + 2FA code (if enabled) + amount above `min_withdrawal`.
  - **AC4:** Locks balance; row inserted `status=processing`; auto-`completed` 30s later.
  - **AC5:** Cancellation allowed while `status=processing`; balance unlocked.

#### FR-WALLET-03: Balance & history

- `GET /account/balances` → all balances with `available`, `locked`, USD value.
- `GET /account/history` → unified deposit + withdrawal + trade ledger view, paginated cursor.

### 7.3 Markets & Listings

#### FR-MKT-01: Market list & metadata

- `GET /markets` → array of markets with `tickSize`, `lotSize`, `minNotional`, `status`, 24h stats (volume, change %, high, low, last price).
- 24h stats refreshed every 5s by API server from a materialised query.

#### FR-MKT-02: Order book snapshot

- `GET /markets/:symbol/orderbook?depth=100` → `{ bids: [[price, qty], ...], asks: [[...]], seq }` ordered best→worst.

#### FR-MKT-03: Recent trades

- `GET /markets/:symbol/trades?limit=50` → newest first.

#### FR-MKT-04: Klines (candles)

- `GET /markets/:symbol/klines?interval=1m|5m|15m|1h|4h|1d&from=&to=` → OHLCV array.
- Supported intervals: `1m, 5m, 15m, 1h, 4h, 1d`. Computed by SQL aggregation over `trades` with `time_bucket` (or hand-written `date_trunc` if no TimescaleDB on Neon free tier).

### 7.4 Trading

#### FR-TRADE-01: Place order

- **Endpoint:** `POST /orders`
- **AC:**
  - **AC1:** Body validated by Rust validator (§ 10.4).
  - **AC2:** `clientOrderId` enforces idempotency: same `(userId, clientOrderId)` returns the original order (with the same `id`), no double-submit.
  - **AC3:** Server-side checks before forwarding to engine:
    - User active, market trading.
    - Quantity ≥ `lot_size` and aligned to `lot_size` step.
    - Price (if present) ≥ `tick_size` and aligned.
    - `price * quantity ≥ min_notional`.
    - For BUY limit: lock `quote = price * quantity * (1 + taker_fee_bps/10000)` from quote-asset `available`.
    - For SELL limit: lock `quantity` from base-asset `available`.
    - For BUY market: require an explicit `quoteQuantity` parameter; lock that from quote.
    - For SELL market: lock `quantity` from base.
    - Stop orders: do not lock until trigger fires; reject with `400 STOP_PRICE_INVALID` if stop in wrong direction.
  - **AC4:** Forward to engine over TCP. Engine response within 50ms or return `503 ENGINE_TIMEOUT`.
  - **AC5:** Engine response includes initial fills (if any); API returns `201` with order state and embedded fills.
  - **AC6:** Errors map to stable codes: `INSUFFICIENT_BALANCE`, `MARKET_HALTED`, `POST_ONLY_REJECTED`, `FOK_NOT_FILLED`, `SELF_TRADE_PREVENTED`, `RATE_LIMITED`, etc.

#### FR-TRADE-02: Cancel order

- `DELETE /orders/:id` and `DELETE /orders?market=&clientOrderId=` (resolve by either).
- **AC:** Idempotent; cancelling an already-final order returns the order with no error.

#### FR-TRADE-03: Cancel-all

- `DELETE /orders?market=BTCUSDT` (no id) cancels all open orders for the user in that market. Without `market`, cancels all across all markets.

#### FR-TRADE-04: Open / closed orders

- `GET /orders?status=open|closed&market=&from=&to=&cursor=&limit=` paginated.

#### FR-TRADE-05: Order types (all eight MUST be supported)

| Type | Behaviour |
|---|---|
| `limit` | Resting at `price`. May fill on entry up to `price`; remainder rests. |
| `market` | Match against book until quantity exhausted or book empty. |
| `ioc` (Immediate-Or-Cancel) | Match available; cancel remainder. |
| `fok` (Fill-Or-Kill) | If full quantity not immediately fillable at acceptable price, cancel entire order, no partial fills. |
| `post_only` | If would cross book on entry, reject (do not match). Else rest. |
| `stop_limit` | Inactive until `stop_price` hit by last trade; then activates as `limit @ price`. |
| `stop_market` | Inactive until trigger; then activates as `market`. |
| `oco` | Pair: one limit + one stop. When one fills, the other cancels. Rejected if both legs invalid. |

Iceberg is implemented as a flag on `limit`: `display_quantity` < `quantity`. Engine reveals `display_quantity` at top of book; refills as fills consume it.

#### FR-TRADE-06: Self-trade prevention (STP)

- Each user has `stp_mode` ∈ {`decrement`, `cancel_maker`, `cancel_taker`} (default `decrement`).
- When taker order would match maker order owned by the same user, engine applies STP:
  - `decrement`: cancel the smaller quantity; the bigger continues with reduced quantity.
  - `cancel_maker`: cancel maker; taker continues against next price level.
  - `cancel_taker`: cancel taker, maker stays.

### 7.5 Public API for Bots

#### FR-API-01: API key management

- `POST /account/api-keys { label, permissions[] }` → returns `{ keyId, secret }` once. Secret stored as `argon2id(secret)`.
- Permissions: `read`, `trade`, `withdraw` (withdraw is hard-disabled in v1, refuses creation with `400`).
- `DELETE /account/api-keys/:keyId` revokes.

#### FR-API-02: HMAC-signed requests

- Header set: `X-AETHER-KEY: <keyId>`, `X-AETHER-TS: <ms>`, `X-AETHER-SIGN: <hex hmac-sha256>`.
- Signature payload: `${ts}\n${method}\n${path}\n${rawBody}`.
- Server rejects if `|now - ts| > 5000ms` (`401 SIG_TIMESTAMP`).
- Server stores `(keyId, ts, sigPrefix)` in Redis with 10s TTL to block replay (`401 SIG_REPLAY`).

#### FR-API-03: Rate limiting

| Tier | Public unauth | User authed (JWT) | API key (HMAC) |
|---|---|---|---|
| Read endpoints | 30 / min / IP | 600 / min | 1200 / min |
| Order place/cancel | n/a | 60 / min | 600 / min |
| Auth endpoints | 10 / 15 min / IP | — | — |
| WS connections | 5 / IP | 20 / user | 20 / key |

Implemented as Redis sliding-window counters. Response headers always include `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`. On limit, `429` with `Retry-After`.

### 7.6 Admin

#### FR-ADM-01: Admin role

- A user with `is_admin=true` (set manually in DB; no UI to grant) can access `/admin` routes.

#### FR-ADM-02: Admin actions

- Halt market: `POST /admin/markets/:symbol/halt` → engine refuses new orders, cancels none. Status `halted`.
- Resume market: `POST /admin/markets/:symbol/resume`.
- Cancel-all in market: `POST /admin/markets/:symbol/cancel-all`.
- Freeze user: `POST /admin/users/:id/freeze` → status=`frozen`; engine cancels their open orders; deposits still credit but cannot trade or withdraw.
- View live engine state: `GET /admin/engine/state` → process metrics, per-symbol order count, top-of-book.

### 7.7 Differentiator Features

#### FR-DIFF-01: Proof-of-Reserves page

- Daily job at 00:00 UTC builds Merkle tree over `(userId, asset, available + locked)` rows.
- Root committed to `proof_of_reserves` table.
- `/proof-of-reserves` page shows root + total liabilities per asset.
- Logged-in users see `Verify my balance` button → opens panel with their leaf, sibling hashes, and verifier code snippet.

#### FR-DIFF-02: Replay mode

- `/markets/:symbol/replay?from=<ts>&speed=1|4|16` page.
- Loads trade history for a 1-minute window; replays into a sandboxed mini-book client-side; user sees price chart and book updates animated at chosen speed. Fully offline UX after load.

#### FR-DIFF-03: Real-time portfolio P&L

- For each asset held, compute FIFO cost basis from the user's trade history.
- `GET /account/pnl` returns `{ asset, qty, avgCost, marketPrice, unrealisedPnl, realisedPnl }` per asset.

#### FR-DIFF-04: Hotkeys

- Global, only when not focused in an input:
  - `b` — focus buy form
  - `s` — focus sell form
  - `1`..`9` — set quantity to 10%..90% of available
  - `Enter` — submit current form
  - `Esc` — clear form
  - `c` — cancel highlighted open order
  - `?` — open hotkey help modal
- Configurable on/off in settings.

---

## 8. Non-Functional Requirements

### 8.1 Performance Targets

| Metric | Target | Measured by |
|---|---|---|
| Engine matching throughput | ≥ 500,000 orders/sec single-thread | `cargo bench -p cex-core` (criterion, in README) |
| Order placement P99 latency (API → engine ack) | ≤ 50 ms | OTel histogram, dashboard |
| WS message fanout P99 | ≤ 75 ms from engine event to client receive | Synthetic prober |
| Book diff size | ≤ 4 KB per symbol per second average | Observed in dev |
| Cold start (engine boot, replay 24h WAL) | ≤ 10s | Bench script |
| API memory | ≤ 256 MB RSS | Fly metrics |
| DB connection pool | 10 per service, max 30 total | Pgbouncer-style limit |

### 8.2 Security (see § 18 for details)

- All connections over TLS in production.
- HSTS, CSP, COOP, COEP headers set on web.
- No secrets in repo; `.env.example` only.
- Argon2id for passwords; sha256 for refresh-token storage.
- HMAC-SHA256 for public API.
- Rate-limited everywhere.
- DB role per service, least privilege.

### 8.3 Accessibility

- WCAG 2.1 AA target.
- Every interactive element keyboard-reachable.
- ARIA roles on order book rows, order forms, modal dialogs.
- Colour contrast ≥ 4.5:1 for text; verified by automated test on every PR.
- Focus rings visible (no `outline:none` without replacement).
- Reduced-motion media query respected on chart animations.

### 8.4 Browser & Device Support

- Latest 2 versions of Chromium, Firefox, Safari.
- Responsive: ≥ 360 px width.
- On viewports < 1024 px, trading screen collapses to tabs (chart / book / form).

### 8.5 Maintainability

- Cyclomatic complexity per function ≤ 10 (enforced by Biome).
- Per-file line limit 400 (warning) / 600 (error).
- All exported functions have JSDoc / rustdoc.
- ADRs (Architecture Decision Records) in `/docs/adr/NNNN-title.md` for any non-obvious choice.

### 8.6 Reliability

- Healthchecks: `/health` (liveness), `/ready` (readiness — DB + engine reachable).
- Graceful shutdown: SIGTERM handler drains in-flight requests up to 25s before exit.
- All non-idempotent client requests **MUST** support `Idempotency-Key` header (alias for `clientOrderId` on order endpoints).

---

## 9. Data Model

> The full DDL is in `infra/migrations/0001_init.sql`. This section is the authoritative spec; the migration MUST match it exactly. If they diverge, the spec wins and the migration is a bug.

### 9.1 Conventions

- All money columns: `NUMERIC(38, 18)`.
- All IDs: `UUID v4`, except `ledger_entries.id` which is `BIGSERIAL` (append rate matters more than ID semantics).
- All timestamps: `TIMESTAMPTZ NOT NULL DEFAULT now()`.
- All foreign keys: `ON DELETE` is explicit (`CASCADE` or `RESTRICT`).
- Soft-deletes are forbidden; instead, status enums (`active|frozen|closed`).

### 9.2 Table Inventory

| Table | Purpose | Append-only? |
|---|---|---|
| `users` | Account record | No |
| `refresh_tokens` | Rotating refresh tokens (hashed) | No |
| `assets` | Listed assets | No |
| `markets` | Listed trading pairs | No |
| `balances` | Current available + locked per (user, asset) | No (in-place updates) |
| `orders` | Order records | No (status updates) |
| `trades` | Match results | **Yes** |
| `ledger_entries` | Every balance mutation | **Yes** |
| `deposits` | Simulated deposits | No |
| `withdrawals` | Simulated withdrawals | No |
| `api_keys` | HMAC keys | No |
| `proof_of_reserves` | Daily Merkle commitments | **Yes** |
| `engine_events` | Replication log of engine output (for settlement re-drive) | **Yes** |
| `audit_log` | Admin actions | **Yes** |

### 9.3 Invariants (enforced by tests)

- **INV-1:** Sum of `ledger_entries.amount` per `(user_id, asset)` equals `balances.available + balances.locked`.
- **INV-2:** For any active order, the locked amount in `balances` is ≥ remaining order obligation.
- **INV-3:** `available >= 0` and `locked >= 0` always (DB CHECK).
- **INV-4:** A `trades` row's `price * quantity` equals the buyer's quote-debit and seller's quote-credit before fees.
- **INV-5:** `orders.filled_quantity` equals the sum of trade quantities for that order.
- **INV-6:** Engine WAL last sequence ≥ DB `engine_events` last sequence (settlement may lag, never lead).
- **INV-7:** No order in status `new|partial` belongs to a user with `status=frozen`.

A nightly job runs all 7 invariants as SQL queries; any failure pages the operator (in production) and breaks CI (in staging).

---

## 10. REST API Contract

### 10.1 Conventions

- Base path: `/api/v1`.
- Content-Type: `application/json` (request and response). Empty body responses are `204`.
- Errors: `{ "error": { "code": "STABLE_CODE", "message": "Human-readable", "details"?: {} }, "requestId": "..." }`.
- All responses include `X-Request-Id`.
- Authentication: `Authorization: Bearer <jwt>` OR HMAC headers (§ 7.5).
- Pagination: cursor-based. Response: `{ "data": [...], "nextCursor": "...|null" }`.

### 10.2 Stable Error Codes (subset; full list in `packages/shared/src/errors.ts`)

`INVALID_EMAIL, WEAK_PASSWORD, EMAIL_TAKEN, INVALID_CREDENTIALS, MFA_REQUIRED, MFA_INVALID, TOKEN_EXPIRED, TOKEN_INVALID, FORBIDDEN, NOT_FOUND, RATE_LIMITED, SIG_TIMESTAMP, SIG_REPLAY, SIG_INVALID, INSUFFICIENT_BALANCE, MARKET_HALTED, MARKET_UNKNOWN, ORDER_NOT_FOUND, POST_ONLY_REJECTED, FOK_NOT_FILLED, SELF_TRADE_PREVENTED, STOP_PRICE_INVALID, LOT_SIZE, TICK_SIZE, MIN_NOTIONAL, ENGINE_TIMEOUT, IDEMPOTENCY_CONFLICT, INTERNAL`

### 10.3 Endpoint Catalogue (summary)

| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/auth/signup` | — | Create user |
| POST | `/auth/login` | — | Login |
| POST | `/auth/mfa` | mfaToken | Submit TOTP |
| POST | `/auth/refresh` | refresh | Rotate tokens |
| POST | `/auth/logout` | jwt | Revoke session |
| POST | `/auth/2fa/setup` | jwt | Begin TOTP enrol |
| POST | `/auth/2fa/verify` | jwt | Confirm TOTP |
| GET | `/account` | jwt | Profile |
| GET | `/account/balances` | jwt | Balances |
| GET | `/account/history` | jwt | Unified history |
| GET | `/account/pnl` | jwt | Real-time P&L |
| POST | `/account/api-keys` | jwt | Create API key |
| DELETE | `/account/api-keys/:id` | jwt | Revoke |
| POST | `/wallet/faucet` | jwt | Mock deposit |
| POST | `/wallet/withdraw` | jwt+2fa | Mock withdrawal |
| DELETE | `/wallet/withdrawals/:id` | jwt | Cancel pending withdrawal |
| GET | `/markets` | — | List |
| GET | `/markets/:symbol` | — | Detail + 24h stats |
| GET | `/markets/:symbol/orderbook` | — | L2 snapshot |
| GET | `/markets/:symbol/trades` | — | Recent trades |
| GET | `/markets/:symbol/klines` | — | Candles |
| POST | `/orders` | jwt or hmac | Place |
| DELETE | `/orders/:id` | jwt or hmac | Cancel |
| DELETE | `/orders` | jwt or hmac | Cancel all (filtered) |
| GET | `/orders` | jwt or hmac | List |
| GET | `/orders/:id` | jwt or hmac | Get |
| GET | `/proof-of-reserves` | — | Latest commitment |
| GET | `/proof-of-reserves/me` | jwt | User leaf + proof |
| POST | `/admin/markets/:symbol/halt` | admin | Halt |
| POST | `/admin/markets/:symbol/resume` | admin | Resume |
| POST | `/admin/users/:id/freeze` | admin | Freeze |
| GET | `/admin/engine/state` | admin | Live engine snapshot |
| GET | `/health` | — | Liveness |
| GET | `/ready` | — | Readiness |
| GET | `/openapi.json` | — | OpenAPI 3.1 spec (generated via utoipa) |

### 10.4 Order Placement — Authoritative Schema

```rust
// services/api/src/schemas.rs
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct PlaceOrderInput {
    #[validate(length(min = 1, max = 64))]
    pub client_order_id: Option<String>,
    pub market: String,
    pub side: String, // mapped to OrderSide Enum in logic
    pub order_type: String, // mapped to OrderType Enum
    pub price: Option<String>,
    pub stop_price: Option<String>,
    pub quantity: Option<String>,
    pub quote_quantity: Option<String>,
    pub display_quantity: Option<String>,
    pub oco_stop_price: Option<String>,
}

// Cross-field validation (e.g. ensuring limit orders have price+quantity) 
// MUST be enforced within a custom #[validate] or a manual validate() implementation.
```

### 10.5 OpenAPI Generation

- `openapi.json` is generated strictly via the `utoipa` crate.
- A CI job diffs the generated spec against the committed snapshot; PR fails if drift detected without snapshot update.

---

## 11. WebSocket API Contract

### 11.1 Connection

- URL: `wss://<api-host>/ws`.
- Subprotocol: none.
- Auth: optional `?token=<jwt>` for private streams; public streams require none.
- Heartbeat: server pings every 20s; client must pong within 30s or be disconnected.

### 11.2 Message Envelope

```json
{ "id": "<uuid>", "method": "subscribe|unsubscribe|ping", "params": { "channels": ["..."] } }
```

Server responses:

```json
{ "id": "<uuid>", "result": { "channels": ["..."] } }
{ "id": "<uuid>", "error": { "code": "...", "message": "..." } }
```

Stream events:

```json
{ "channel": "<name>", "seq": 12345, "data": { ... } }
```

### 11.3 Public Channels

| Channel | Format |
|---|---|
| `book.<symbol>.snapshot` | Sent once on subscribe. `{ bids, asks, seq }` |
| `book.<symbol>.diff` | `{ bids: [[price,qty]], asks: [[price,qty]], seq }`. Quantity `0` means "remove that price level". |
| `trade.<symbol>` | `{ id, price, qty, side, ts }` |
| `kline.<symbol>.<interval>` | `{ open, high, low, close, volume, ts }` (closed klines) |
| `ticker.<symbol>` | 24h rolling stats every 1s |
| `ticker.all` | All markets ticker, batched every 1s |

### 11.4 Private Channels (require token)

| Channel | Format |
|---|---|
| `user.orders` | Order updates for the authed user |
| `user.fills` | Fills (trade events involving the user) |
| `user.balances` | Balance updates |

### 11.5 Sequence & Resync Protocol

- Each public channel has a monotonic `seq` per symbol.
- On reconnect, client may send `{"method":"subscribe", "params":{"channels":["book.BTCUSDT"], "afterSeq": 12340}}`.
  - If server still has the diffs in its buffer (last 60s, in Redis), it replays them.
  - If gap is too large, server responds `{ "error": { "code": "GAP_TOO_LARGE" } }` and client must resnapshot.

### 11.6 Backpressure

- If a client's send buffer exceeds 1 MB, server closes connection with code `1013`.

---

## 12. Engine ↔ API Wire Protocol

> Versioned. v1 spec frozen. Future v2 adds gRPC; v1 remains supported for one major version.

### 12.1 Transport

- TCP, loopback only (`127.0.0.1:7878`).
- Length-prefixed JSON: each message is `<u32 BE length><utf8 json>`.
- One connection per API process; multiplexed by `requestId` (uuid).

### 12.2 Request types (API → Engine)

```ts
type EngineRequest =
  | { kind: 'place'; requestId: string; order: EngineOrder }
  | { kind: 'cancel'; requestId: string; symbol: string; orderId: string }
  | { kind: 'cancelAll'; requestId: string; userId: string; symbol?: string }
  | { kind: 'haltMarket'; requestId: string; symbol: string }
  | { kind: 'resumeMarket'; requestId: string; symbol: string }
  | { kind: 'snapshot'; requestId: string; symbol: string; depth: number }
  | { kind: 'ping'; requestId: string };
```

### 12.3 Response types (Engine → API)

```ts
type EngineResponse =
  | { kind: 'ack'; requestId: string; result: { orderId: string; status: OrderStatus; fills: Fill[]; seq: number } }
  | { kind: 'reject'; requestId: string; code: string; message: string }
  | { kind: 'event'; seq: number; event: EngineEvent }      // unsolicited
  | { kind: 'pong'; requestId: string };
```

### 12.4 Engine Events (broadcast to all connected API processes)

```ts
type EngineEvent =
  | { type: 'orderAccepted'; order: EngineOrder }
  | { type: 'orderRested'; orderId: string; remaining: string }
  | { type: 'fill'; tradeId: string; symbol: string; takerOrderId: string; makerOrderId: string;
      price: string; quantity: string; takerSide: 'buy'|'sell'; ts: string }
  | { type: 'orderCanceled'; orderId: string; reason: string }
  | { type: 'bookDelta'; symbol: string; bids: [string, string][]; asks: [string, string][]; seq: number }
  | { type: 'marketStatus'; symbol: string; status: 'trading' | 'halted' };
```

### 12.5 Ordering & Durability

- Engine writes each event to its WAL **before** broadcasting.
- API receives events out of band; settlement worker subscribes to a Postgres-backed copy.
- Sequence numbers are global across all symbols (single counter), monotonically increasing by 1.

---

## 13. Order Lifecycle & State Machine

```
         ┌───────────────────────────┐
         │  client submits order     │
         └────────────┬──────────────┘
                      │ API validation
                ┌─────┴─────┐
        rejected│           │accepted
                ▼           ▼
            REJECTED       NEW ──────────┐
                            │            │
                  fully     │   partial  │ canceled
                  filled    │   filled   │
                            ▼            ▼
                          FILLED       PARTIAL ─┬─ further fill ─→ FILLED
                                                │
                                                └─ cancel ────────→ CANCELED
```

### 13.1 Status Definitions

| Status | Meaning |
|---|---|
| `new` | Resting on book, not yet matched |
| `partial` | Resting, partially filled |
| `filled` | Fully filled |
| `canceled` | Canceled (by user, admin, IOC/FOK rules, or STP) |
| `rejected` | Validation or risk rejected; never reached engine OR engine refused (post-only crossing, etc.) |

### 13.2 Transitions

- `new → partial`: a fill consumed some quantity, more remains
- `new → filled`: a fill consumed all quantity
- `new → canceled`: user/admin cancel, or IOC/FOK
- `partial → filled`: subsequent fill completes
- `partial → canceled`: user/admin cancel
- All terminal: `filled`, `canceled`, `rejected`

Stop orders introduce `pending` (pre-trigger). Transitions: `pending → new` on trigger, `pending → canceled` if user cancels before trigger.

OCO orders: when one leg becomes terminal (`filled` or `canceled`), the other auto-`canceled` with `reason='oco'`.

---

## 14. Matching Engine Specification

This is the contract the Rust implementation must satisfy. Tested by a conformance suite (§ 14.7).

### 14.1 Data Structures

- One **per-symbol actor** owning the book; orders for symbol routed through a single-producer queue → no locks on the hot path.
- Book = `(BTreeMap<Price, VecDeque<Order>>, BTreeMap<Price, VecDeque<Order>>)` for bids (descending) and asks (ascending).
- All numbers: `rust_decimal::Decimal`. `f64` is forbidden in matching code; `clippy::float_arithmetic` lint set to `deny`.

### 14.2 Price-Time Priority

- Bids match best (highest) first; FIFO within a level.
- Asks match best (lowest) first; FIFO within a level.

### 14.3 Match Algorithm (limit order entering)

```text
function match(taker):
    book = books[taker.symbol]
    counterSide = book.opposite(taker.side)
    while taker.remaining > 0 and counterSide.bestPrice exists:
        if taker is limit and counterSide.bestPrice does not satisfy taker.price:
            break
        maker = counterSide.bestPriceLevel.front()
        if stp_applies(taker, maker):
            apply_stp(taker, maker); continue
        qty = min(taker.remaining, maker.remaining)
        emit Fill(taker, maker, maker.price, qty)
        taker.remaining -= qty
        maker.remaining -= qty
        if maker.remaining == 0:
            counterSide.bestPriceLevel.popFront()
            if counterSide.bestPriceLevel.empty:
                counterSide.removeLevel()
    if taker.remaining > 0:
        if taker is market or ioc: emit Cancel(taker, "no_liquidity")
        elif taker is fok and any fill happened: panic // pre-checked, see § 14.4
        elif taker is fok: emit Cancel(taker, "fok_unfilled")
        elif taker is post_only and any fill happened: panic // pre-checked
        else:
            book.sameSide.insert(taker)  # rest at price level
            emit Rested(taker)
```

### 14.4 FOK and Post-Only Pre-Checks

Both types must be **inspected before mutation**:

- **FOK:** walk the book and sum quantity available at acceptable prices. If < taker quantity, reject without any state change.
- **Post-only:** if best opposite price would cross taker's price, reject without any state change.

### 14.5 Stop Orders

- Stops live in a separate `BTreeMap<TriggerPrice, Vec<Order>>` per direction.
- After every trade, check stops:
  - Buy stops with `stopPrice <= lastTradePrice` trigger.
  - Sell stops with `stopPrice >= lastTradePrice` trigger.
- Triggered stops convert to `limit` (their `price`) or `market` and re-enter the matching loop.

### 14.6 Persistence (WAL)

- Every accepted order, cancel, and emitted fill is appended to a write-ahead log file before being broadcast.
- WAL format: length-prefixed bincode-serialised events.
- On engine boot:
  1. Load latest snapshot file (binary, opaque, every 60s).
  2. Replay WAL since snapshot.
  3. Subscribe to API; resume.
- If WAL is corrupt at the tail, truncate to last good record and log a `wal.recovery` event.

### 14.7 Conformance Test Suite (mandatory)

Implemented as `engine/crates/core/tests/conformance.rs`, **at least 40 cases**:

- 1: Empty book, market buy → cancel `no_liquidity`.
- 2: One ask at `100×1`, market buy `1` → fill `100×1`, both orders gone.
- 3: Crossed-book is impossible after every operation.
- 4: Limit buy `100×2` matches asks `99×1, 100×1` → two fills at maker prices.
- 5: FOK buy `100×3` against asks `99×1, 100×1` → reject, no fill.
- 6: Post-only buy `100` when best ask is `100` → reject crossing.
- 7: Iceberg ask `100×10 display 2`: 5 buys of `2` cause 5 fills, only `2` ever shows on book.
- 8: STP `decrement`: same user limits cross → smaller cancelled.
- 9: STP `cancel_maker`: maker cancelled, taker continues.
- 10: STP `cancel_taker`: taker cancelled, maker stays.
- 11: Stop-buy `stop=100, price=101` triggers when last trade ≥ 100; converts to limit `101`.
- 12–40: combinatorial coverage of side × type × edge cases (zero qty, max qty, dust below `min_notional`, partial fills across many price levels, cancel during partial, cancel-all in symbol).
- Property tests with `proptest`: 10,000 random sequences; book invariants checked after each step.

### 14.8 Performance

- `cargo bench` target on M2/Ryzen 7 baseline:
  - `bench_match_random_limit_orders`: ≥ 500,000 ops/sec.
  - `bench_book_snapshot_depth_100`: ≥ 100,000 ops/sec.
- Benchmarks committed in `engine/crates/core/benches/`. README shows latest numbers.

---

## 15. Settlement & Ledger Rules

### 15.1 Trigger

Settlement worker is an independent process consuming engine events from the `engine_events` table (Postgres LISTEN/NOTIFY pushes new rows; worker uses `SELECT FOR UPDATE SKIP LOCKED` for parallelism-safety even with one instance, ready for scale).

### 15.2 Idempotency

Each `engine_events` row has a unique `seq`. Worker tracks `last_processed_seq` in `worker_state` table. Reprocessing a row is a no-op (the writes are themselves keyed by `seq`).

### 15.3 Trade Settlement (the canonical case)

Given a `fill` event `{ takerOrderId, makerOrderId, price, qty, takerSide }`:

```
BEGIN TRANSACTION;
  -- 1. Write the trade.
  INSERT INTO trades (...) VALUES (...);

  -- 2. For BUY-side trader: receives base, pays quote.
  --    The 'locked' decreases by min(price*qty*(1+takerFeeBps/10000), remaining lock).
  --    Fees are debited from the asset received (Binance convention).
  ledger_credit(buyer, base_asset, qty - taker_fee_in_base);
  ledger_debit_locked(buyer, quote_asset, price * qty);
  ledger_fee(buyer, base_asset, taker_fee_in_base);

  -- 3. For SELL-side trader: receives quote, pays base.
  ledger_credit(seller, quote_asset, price * qty - maker_fee_in_quote);
  ledger_debit_locked(seller, base_asset, qty);
  ledger_fee(seller, quote_asset, maker_fee_in_quote);

  -- 4. Update orders.filled_quantity, status, avg_fill_price.
  UPDATE orders SET ... WHERE id IN (taker, maker);

  -- 5. Mark seq processed.
  UPDATE worker_state SET last_processed_seq = $seq;
COMMIT;
```

Each `ledger_*` helper writes a row to `ledger_entries` and updates `balances`, atomic within the transaction.

### 15.4 Cancel Settlement

Refund the unused `locked` to `available`; write ledger entry `kind=unlock`.

### 15.5 Fee Schedule

- Per-market, in `markets.maker_fee_bps` and `markets.taker_fee_bps`. Default 10 bps maker / 10 bps taker. Configurable.

---

## 16. Market-Maker Bot Specification

### 16.1 Purpose

Provide realistic depth on all listed markets without real users. **Clearly disclosed in UI**.

### 16.2 Behaviour

- One synthetic user account: `mm@aether.local`, flagged `is_mm=true`. Excluded from Proof-of-Reserves leaf set with disclosure.
- For each listed market:
  - Subscribes to Binance `<symbol>@bookTicker` (best bid/ask).
  - Maintains 10 bid levels and 10 ask levels at `mid * (1 ± k * spreadBps/10000)` for k ∈ 1..10.
  - Quantities chosen log-normally to mimic realistic depth.
  - Refresh every 2 seconds.
  - Cancels and replaces stale levels.
- If Binance feed lags > 30s, MM stops quoting (book "thins out") rather than quoting stale prices.

### 16.3 Limits

- MM account funded with `1e12` of every asset at boot via a one-time admin migration (clearly logged).
- MM fees credited to a separate `fees` user, not retained as MM PnL (so balances stay clean).

### 16.4 Disclosure

- Every market's order book has a small "ⓘ" tooltip: "Liquidity provided by demo market-maker bot for portfolio purposes."

---

## 17. Frontend Specification

### 17.1 Reference Design

Backpack Exchange (https://backpack.exchange/). Match the **information density and dark-mode palette**, not the exact pixels. Original logo and colour accents.

### 17.2 Routes (App Router)

```
/                                  — landing (marketing-light, top markets snapshot)
/markets                           — all markets, sortable, searchable
/trade/:symbol                     — main trading screen
/portfolio                         — balances, P&L, history
/wallet                            — deposit/withdraw, transactions
/account                           — profile, security, API keys
/proof-of-reserves                 — public PoR
/markets/:symbol/replay            — replay mode
/admin                             — admin only (gated by RSC server check)
/auth/(signup|login|2fa|...)       — auth flows
/legal/(terms|privacy|disclaimer)  — static pages
```

### 17.3 Trading Screen Layout (≥ 1280 px viewport)

```
┌─────────────────────────────────────────────────────────────────────┐
│ Top bar: logo · market selector · 24h stats · search · account      │
├──────────────┬───────────────────────────────────┬──────────────────┤
│              │                                   │                  │
│  Order book  │                                   │   Order form     │
│  (live)      │     TradingView chart             │   (side tabs)    │
│              │                                   │                  │
│  Recent      │                                   │   Open orders    │
│  trades      │                                   │   Order history  │
│              │                                   │   Trade history  │
└──────────────┴───────────────────────────────────┴──────────────────┘
```

- Three columns at ≥ 1280 px; tabs at < 1280 px; single-column with sheet at < 768 px.
- All panels resize with persisted ratios (localStorage).
- Theme: dark default (`bg-zinc-950`), light theme available, system preference respected.

### 17.4 Component Behaviour Specs

#### 17.4.1 Order Book

- Up to 25 levels each side; cumulative depth bar coloured (green bids, red asks) at 8% opacity.
- Updates from `book.<symbol>.diff` channel; no full re-render — uses keyed rows.
- Click a row → fills price into order form.
- Hover a row → shows cumulative size and notional.
- Spread shown between sides (bps + absolute).
- Aggregation toggle: 1×, 10×, 100× tick.

#### 17.4.2 Chart

- `lightweight-charts` v4.
- Intervals: 1m, 5m, 15m, 1h, 4h, 1d.
- Live updates from `kline.<symbol>.<interval>` and `trade.<symbol>` (in-progress candle).
- Crosshair shows OHLC.
- No drawing tools in v1 (clearly out of scope to keep timeline).

#### 17.4.3 Order Form

- Tabs: Buy / Sell.
- Type selector: Market | Limit | Stop | Stop-Limit | OCO | Iceberg (each toggling visible fields).
- Quantity input with quick-percent buttons (25/50/75/100) of available.
- "Total" computed live; honours `tick_size`, `lot_size`, `min_notional`.
- Validation errors shown inline; submit disabled until valid.
- After submit: optimistic add to "Open orders"; rolled back on failure.

#### 17.4.4 Open Orders / History

- Real-time updates from `user.orders` channel.
- Each row: cancel button. Cancel-all button at top.
- Filters: market, type, side, time.

#### 17.4.5 Hotkeys

Implemented as a single `useHotkeys` hook reading from a typed map. Not bound when input has focus.

### 17.5 Accessibility Specs

- Order book rows: `role="row"`, prices have `aria-label="Bid 100.50, size 2.5 BTC"`.
- Order form: properly labeled inputs; errors via `aria-describedby`.
- Modals: focus trap, `aria-modal="true"`, ESC closes.
- Skip-to-main-content link.
- Live regions: `role="status"` for toasts, `aria-live="polite"` for new fills.

### 17.6 Performance Specs

- LCP ≤ 1.8s on Fast 3G.
- TBT ≤ 150ms.
- CLS ≤ 0.05.
- Verified via Lighthouse CI on every PR; thresholds enforced.

### 17.7 Empty / Loading / Error States

Every async surface has all four states explicit:
1. **Loading** (skeleton, never spinner-only)
2. **Empty** (helpful CTA)
3. **Error** (retry button, actionable copy)
4. **Success/data**

A reusable `<AsyncBoundary>` component encapsulates this; every page MUST use it.

---

## 18. Security Model

### 18.1 Threats In Scope

| Threat | Mitigation |
|---|---|
| Credential stuffing | Rate limit 5/15min per email+IP; argon2id; 2FA |
| Token theft | Short access TTL; refresh rotation with reuse detection; httpOnly+Secure+SameSite=Lax cookies for session |
| CSRF | API uses `Authorization` header (not cookie) for JWT → CSRF n/a for API. Web→API uses SameSite cookies + double-submit token for any cookie-auth path |
| XSS | React default escaping; CSP `script-src 'self' 'wasm-unsafe-eval'`; no `dangerouslySetInnerHTML` outside controlled markdown |
| SQL injection | All queries via sqlx parameterisation; raw SQL forbidden outside `infra/migrations` |
| Replay (signed API) | `X-AETHER-TS` ±5s window; redis dedupe; nonce optional |
| Rate-limit bypass | Per-IP + per-user + per-key; Redis sliding window; `Retry-After` populated |
| Order spoofing | All orders authed; engine refuses unauthed |
| Self-trade & wash | STP enforced; admin dashboard surfaces suspicious volume |
| Internal abuse | DB role per service; only `settlement` can write `balances`; only `api` reads them |
| Secret leakage | `.env` git-ignored; CI scans for AWS/GitHub key patterns; pre-commit blocks |
| Dependency CVEs | Dependabot + `npm audit` + `cargo audit` in CI; high/critical fail the build |

### 18.2 Threats Out of Scope (documented)

- Real custody attacks (no funds).
- Sybil/airdrop farming (not a real platform).
- Phishing of the user (out of platform's control).

### 18.3 Security Headers

```
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
Content-Security-Policy: default-src 'self'; img-src 'self' data: https:; script-src 'self'; connect-src 'self' wss://<api> https://api.coingecko.com; frame-ancestors 'none'; base-uri 'self'; form-action 'self'
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=()
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
```

### 18.4 HMAC Spec (full)

- Algorithm: `HMAC-SHA256`.
- Header names: `X-AETHER-KEY`, `X-AETHER-TS` (ms epoch), `X-AETHER-SIGN` (lowercase hex).
- Signed payload: `{ts}\n{METHOD}\n{path}\n{rawBody||''}`.
- Server computes; compares with `crypto.timingSafeEqual`.
- Replay window: ±5000 ms.
- Replay storage: Redis `SETEX hmac:<keyId>:<sigPrefix12> 10 1`; if exists → `SIG_REPLAY`.
- Permission check: order endpoints require `trade` permission; account-write requires `read` minimum.

### 18.5 DB Roles (Postgres)

- `cex_owner` — migrations only, never used by services.
- `cex_api` — `SELECT, INSERT, UPDATE` on most tables; **no DELETE**, **no truncate**.
- `cex_settlement` — `SELECT, INSERT, UPDATE` on `balances`, `ledger_entries`, `trades`, `orders`; nothing on `users`.
- `cex_market_data` — `SELECT, INSERT` on `klines`, `markets` updates only.

---

## 19. Observability & Operations

### 19.1 Logs

- `tracing` JSON logs. Levels: `trace, debug, info, warn, error, fatal`.
- Required fields on every log line: `time, level, service, env, version, requestId, userId?, event`.
- PII redaction: `password, totpSecret, secret, authorization, cookie, refreshToken` always redacted by config.
- Local dev: pretty-printed via `tracing-subscriber` with `fmt` feature.
- Production: JSON to stdout, scraped by Grafana Cloud agent.

### 19.2 Metrics (Prometheus / OTel)

| Name | Type | Labels |
|---|---|---|
| `http_requests_total` | counter | method, route, status |
| `http_request_duration_seconds` | histogram | method, route |
| `engine_orders_processed_total` | counter | symbol, type, result |
| `engine_match_duration_seconds` | histogram | symbol |
| `engine_book_depth` | gauge | symbol, side |
| `ws_connections` | gauge | — |
| `ws_messages_sent_total` | counter | channel |
| `settlement_lag_seq` | gauge | — (engine_seq − processed_seq) |
| `db_pool_in_use` | gauge | service |

### 19.3 Tracing

- OpenTelemetry SDK in API + Engine + Settlement.
- One trace per HTTP request; spans: validate → auth → engine → response.
- Engine spans link to API parent via injected `traceparent` field in TCP request.

### 19.4 Dashboard

- Grafana dashboard JSON at `infra/observability/dashboards/aether.json`. Panels:
  - HTTP RPS / P50 / P99
  - Engine orders/sec
  - Settlement lag
  - Active WS
  - DB pool
  - Errors per minute
- Screenshot in README, commit count `≥ 1` to dashboard JSON before "done".

### 19.5 Runbooks

`docs/runbooks/`:
- `engine-restart.md`
- `settlement-stuck.md`
- `db-migration.md`
- `incident-response.md`

---

## 20. Testing Strategy

### 20.1 Test Pyramid

| Layer | Scope | Target |
|---|---|---|
| Unit (Rust + TS) | Pure functions, modules | ≥ 80% line coverage |
| Property (Rust) | Engine invariants | 10,000 cases per invariant |
| Integration (TS) | API + DB + engine via Docker | All happy-path endpoints |
| Conformance (Rust) | Engine spec § 14.7 | 40+ cases, all green |
| E2E (Playwright) | Full user journeys | 5 scenarios (§ 20.4) |
| Lighthouse CI | Web perf | ≥ 90 PWA, ≥ 95 a11y, ≥ 90 perf, ≥ 95 SEO |
| Load (k6) | API smoke, 100 concurrent | Median < 100ms |

### 20.2 Engine Property Tests (mandatory cases)

- After any sequence of placements/cancels: `bestBid < bestAsk` (no crossed book).
- After any sequence: `sum(orders.remaining) == sum(book quantities at all levels)`.
- Replaying WAL produces identical book state.
- Cancel-all in symbol leaves zero orders.

### 20.3 Integration Test Setup

- Docker compose spins Postgres + Redis + Engine.
- Vitest with `setupFiles` running migrations and seeding `assets`/`markets`.
- Each test file truncates tables in `afterEach` (not drop, faster).

### 20.4 E2E Scenarios (Playwright)

- **E2E-01:** Sign up → enable 2FA → log in with 2FA → land on trading screen.
- **E2E-02:** Faucet 1000 USDT → place limit buy → see open order in panel → cancel → balance restored.
- **E2E-03:** With prefilled balance, place market buy → see fill toast → balance updated → trade in history.
- **E2E-04:** Public REST: signup, generate API key, send signed `POST /orders`, receive `201`.
- **E2E-05:** Replay mode: open `/markets/BTCUSDT/replay?from=...&speed=4` → press play → chart animates.

### 20.5 Forbidden Test Patterns

- `await page.waitForTimeout(...)` — use locators with auto-wait.
- `it.skip` or `it.only` in committed code (Biome rule blocks).
- Snapshot tests of large rendered HTML — too brittle; explicit assertions only.

---

## 21. 14-Day Delivery Plan

> Honest, sustainable pace. ~9 hrs effective work / day. Evenings can compress D11–D14 but D1–D7 should not be heroic.

### Phase 1 — Foundations (Days 1–2)

**Day 1 (foundations & infra)**
- [ ] Workspace: pnpm + Cargo, root configs (Biome, tsconfig, .nvmrc, husky)
- [ ] Postgres schema (`0001_init.sql`) + seed (`0002`)
- [ ] Docker compose (Postgres, Redis, Jaeger) with healthchecks
- [ ] CI pipeline (.github/workflows/ci.yml) — TS + Rust + e2e jobs
- [ ] `scripts/dev.sh`, `scripts/e2e.sh`
- [ ] `.env.example` complete
- [ ] README skeleton committed

**Day 2 (engine core part 1)**
- [ ] `cex-core` crate: `Order`, `Side`, `OrderType`, `Book`, basic insert/cancel
- [ ] Match algorithm for `limit` + `market`
- [ ] Property tests for book invariants
- [ ] First criterion benchmark scaffold

### Phase 2 — Engine & Settlement (Days 3–5)

**Day 3 (engine core part 2)**
- [ ] IOC, FOK, post-only, stop, stop-limit, OCO, iceberg
- [ ] Self-trade prevention (3 modes)
- [ ] WAL append + replay
- [ ] Conformance suite (40 cases)
- [ ] Bench number ≥ 500k ops/sec

**Day 4 (engine server + API skeleton)**
- [ ] `cex-server` binary: TCP listener, framed JSON, request/response routing, event broadcast
- [ ] `services/api`: Axum boot, healthz/readyz, structured logging, error envelope, request-id middleware
- [ ] Auth: signup, login, JWT issue + refresh rotation, argon2id
- [ ] validator-driven validation; `@cex/shared` package

**Day 5 (settlement + market data)**
- [ ] `services/settlement`: consume engine events via `engine_events` table + LISTEN/NOTIFY; write balances and ledger atomically
- [ ] Invariants test: simulate 1000 fills and verify INV-1..7 hold
- [ ] `services/market-data`: Binance WS ingest for top 50 USDT pairs; klines persistence
- [ ] Market-maker bot: 10×10 quoting around real mid

### Phase 3 — API surface (Days 6–7)

**Day 6 (trading endpoints + WS)**
- [ ] All `/orders` endpoints (place, cancel, list, get); idempotency
- [ ] All `/markets/*` endpoints
- [ ] `/account/*` endpoints
- [ ] WS hub: subscribe/unsubscribe, public channels (`book`, `trade`, `kline`, `ticker`), private channels (`user.orders`, `user.fills`, `user.balances`)
- [ ] Snapshot+diff protocol with seq numbers and resync

**Day 7 (security & rate limits)**
- [ ] Rate limiting (Redis sliding window) for all endpoint classes
- [ ] HMAC signature verification middleware
- [ ] API key CRUD endpoints
- [ ] All security headers configured
- [ ] OpenAPI generation via utoipa
- [ ] 2FA full flow

### Phase 4 — Frontend (Days 8–11)

**Day 8 (frontend shell)**
- [ ] Next.js app, Tailwind, theme, layout, top bar, market selector
- [ ] Auth pages (signup, login, 2fa, forgot)
- [ ] TanStack Query setup, ws client hook with reconnect+resume

**Day 9 (trading screen part 1)**
- [ ] Order book component (subscribe + diff merge + virtualised rows)
- [ ] Recent trades component
- [ ] Chart (lightweight-charts) with klines + live updates

**Day 10 (trading screen part 2)**
- [ ] Order form (all 8 types, validation, submit, optimistic update)
- [ ] Open orders / order history / trade history panels
- [ ] Hotkey system

**Day 11 (portfolio + wallet + account)**
- [ ] Portfolio page (balances, P&L)
- [ ] Wallet (faucet, withdraw form with address validation)
- [ ] Account (profile, 2FA management, API keys)

### Phase 5 — Differentiators & Polish (Days 12–14)

**Day 12 (differentiators)**
- [ ] Proof-of-Reserves: nightly job + page + verify-my-balance flow
- [ ] Replay mode page
- [ ] Real-time P&L computation (FIFO)
- [ ] Admin console (halt market, freeze user, engine state)

**Day 13 (observability + deploy)**
- [ ] OTel everywhere (traces); Grafana dashboard JSON committed
- [ ] Lighthouse CI thresholds enforced
- [ ] Fly.io deploy (`infra/fly/`) + Vercel deploy + Neon DB
- [ ] Smoke E2E against deployed environment

**Day 14 (E2E + readme + demo)**
- [ ] All 5 Playwright E2Es green in CI
- [ ] README finalised: video, architecture, benchmark, demo URL, scope table, roadmap
- [ ] 90-second Loom demo recorded and embedded
- [ ] Tag `v1.0.0`

### Daily Rituals

- Start: review yesterday's checklist; pick today's; write one-line goal in `docs/journal.md`.
- End: commit, push, update `CHANGELOG.md`, write one-line outcome.
- If blocked > 30 min on the same problem, write a paragraph in `docs/journal.md` describing the block before continuing or seeking help.

---

## 22. Clarification Protocol

When an AI agent or human implementer encounters an ambiguity:

1. **Search this PRD** (Cmd+F) for the relevant noun. If found, follow it.
2. If not found, search `AGENTS.md` and `/docs/adr/`.
3. If still not found, **stop and add a `// PRD-AMBIGUITY:` comment** at the call site with a precise question.
4. Open `docs/clarifications.md` and append:

```
## YYYY-MM-DD — <one-line topic>
**Context:** path/to/file.ts:42
**Question:** ...
**Proposal A:** ...
**Proposal B:** ...
**Recommendation:** A/B with rationale.
```

5. Pick the recommendation, proceed.
6. After resolution, **update this PRD** in the relevant section so the question never recurs.

**Forbidden:** silently picking a behaviour and shipping without recording the choice.

---

## 23. Acceptance Criteria & Definition of Done

A user story is "done" only when **all** of the following are true:

1. Code is merged to `main` via PR.
2. PR description references the FR-id(s).
3. CI is green: lint, typecheck, unit, integration, conformance, E2E, Lighthouse.
4. Test coverage on changed lines ≥ 80% (Codecov delta).
5. Manual smoke pass: the human writes "tested locally + deployed" in the PR.
6. Logs and metrics are emitted for the new code path (verified in dashboard).
7. README or `docs/` updated if user-facing or operator-facing behaviour changed.
8. No `// TODO` left without a referenced GitHub issue.
9. `CHANGELOG.md` entry under `## [Unreleased]` describing the change.

The project as a whole is "v1.0 done" when:

- All FR-* in § 7 are done.
- All NFR-* in § 8 are met or documented as deferred (with reason).
- All 5 E2Es pass in CI against the deployed environment.
- The benchmark number is published in README.
- The 90-second Loom is linked.
- The `v1.0.0` git tag is pushed.

---

## 24. Risks & Mitigations

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Rust learning curve slows engine | High | High | Property tests guard against subtle bugs; AI-assist for boilerplate; engine scope limited to ~3k LOC |
| R2 | Free tiers throttle on demo day | Med | Med | Pre-warm 1h before recruiter view; fallback Loom video |
| R3 | Binance WS rate-limit cuts feed | Low | Med | Reconnect-with-backoff; cached last good prices for ≤ 60s |
| R4 | WebSocket gap recovery is buggy | Med | High | Aggressive resync test; client falls back to full resnapshot if any anomaly |
| R5 | Decimal arithmetic mistakes | Med | Critical | Forbid floats; type-level "Money" wrapper in TS; `clippy::float_arithmetic` deny in Rust |
| R6 | Settlement falls behind engine | Med | High | `settlement_lag_seq` metric + alert; backpressure: reject new orders if lag > 1000 |
| R7 | Scope creep eats Day 13–14 | High | Med | Hard freeze on FR list at end of Day 7; new ideas → `docs/v2-ideas.md` |
| R8 | E2E flakiness in CI | Med | Low | Use locator auto-wait; no `waitForTimeout`; test isolation via DB truncate |
| R9 | Recruiters don't open the repo | Low | Critical | LinkedIn post + 90s video + benchmark numbers in first scroll |

---

## 25. Glossary

| Term | Definition |
|---|---|
| **Aether** | Project codename. |
| **AC** | Acceptance Criterion. |
| **API** | The Axum Rust service in `services/api`. |
| **Book** | The order book for a single symbol. |
| **BPS** | Basis points; 1 bps = 0.01%. |
| **CEX** | Centralised Exchange. |
| **Engine** | The Rust matching engine in `engine/`. |
| **Fill** | A match between a taker and maker; produces a trade. |
| **FOK** | Fill-Or-Kill. |
| **HMAC** | Hash-based Message Authentication Code. |
| **IOC** | Immediate-Or-Cancel. |
| **Kline** | OHLCV candle. |
| **Lot size** | Minimum quantity increment for a market. |
| **Maker** | Order resting on the book that gets matched. |
| **MM** | Market Maker (the bot). |
| **MVP** | Not used in this project; we ship v1.0 (full-scope), not MVP. |
| **OCO** | One-Cancels-Other. |
| **PnL** | Profit and Loss. |
| **PoR** | Proof-of-Reserves. |
| **Snapshot+diff** | Streaming protocol where a one-time snapshot precedes incremental deltas. |
| **STP** | Self-Trade Prevention. |
| **Taker** | Order that crosses the book and consumes liquidity. |
| **Tick size** | Minimum price increment for a market. |
| **WAL** | Write-Ahead Log. |

---

**End of PRD v1.0.0.** Any change requires a PR titled `prd: <topic>` with a `CHANGELOG.md` entry under `## [Unreleased] - PRD`.
