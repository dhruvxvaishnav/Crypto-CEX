# Project Progress

This local file is intentionally git-ignored. Use it as a scratch progress board during implementation sessions.

## Foundations

- [x] Initial AGENTS and PRD created
- [x] Frontend latest-version decision accepted by owner
- [x] Day 1 foundation verified locally

## Engine Core (Day 2 Part 1)

- [x] Defined core data structures (`Order`, `Side`, `OrderType`, `Fill`) in `types.rs`
- [x] Implemented core order book and matching logic for Limit/Market orders in `book.rs`
- [x] Added `proptest` for book invariants (no crossed book, accurate depth tracking)
- [x] Set up criterion benchmarks hitting > 5 million ops/sec (target: 500,000 ops/sec)

## Engine Core (Day 2 Part 2)

- [x] `StpMode` (Decrement/CancelMaker/CancelTaker), `CancelReason` enum, extended `OrderType` (Ioc/Fok/PostOnly/StopLimit/StopMarket/Oco/Iceberg) and `Order` fields (`stp_mode`, `stop_price`, `oco_linked_id`, `display_qty`) in `types.rs`
- [x] `Order.symbol` changed to `Arc<str>` for O(1) clone — faster snapshot bench
- [x] `StopRegistry` in `stop.rs`: buy/sell stop trigger logic per PRD §14.5
- [x] WAL in `wal.rs`: length-prefixed bincode, `Wal::append/replay`, `Snapshot::save/load`
- [x] `book.rs` extended: STP (3 modes), FOK pre-check, post-only pre-check, IOC, iceberg re-queue, stop cascade (depth-guarded), `cancel_order`, OCO link cancellation, `register_oco`, `rebuild_index`
- [x] `conformance.rs`: 45 cases — all green
- [x] `properties.rs`: extended `arb_order` with new fields + `Ioc` variant
- [x] `bincode = "1"` added to workspace for WAL serialization (PRD §14.6)
- [x] `rust_decimal` feature `serde-str` added (fixes bincode Decimal deserialization; correct per AGENTS.md §3.13)
- [x] `serde` feature `rc` added for `Arc<str>` serialization
- [x] AGENTS.md §2: `ProjectProgress.md` added as item 0 in the source-of-truth hierarchy

### Benchmarks (Day 2 Part 2)

| Benchmark | Result | Target | Multiple |
|---|---|---|---|
| `bench_match_random_limit_orders` | 2.98M ops/sec | ≥ 500k | 5.96× |
| `bench_book_snapshot_depth_100` | 129k ops/sec | ≥ 100k | 1.29× |

## Engine Server (Day 4A)

- [x] `cex-proto` now owns PRD §12 wire types for `EngineRequest`, `EngineResponse`, `EngineEvent`, order/status enums, snapshots, and length-prefixed JSON framing
- [x] `cex-server` TCP listener runs on loopback, routes framed requests, persists commands to WAL, persists sequenced events before broadcast, and broadcasts unsolicited events to connected API clients
- [x] Implemented `ping`, `place`, `cancel`, `cancelAll`, `snapshot`, `haltMarket`, and `resumeMarket` routing against the existing `cex-core` order books
- [x] Added L2 depth snapshots via `OrderBook::depth_levels`
- [x] Added TCP integration coverage for ping, resting order placement + snapshot, and cancel + snapshot removal

## API Skeleton + Auth (Day 4B)

- [x] Added `services` Rust workspace with `cex-api` Axum service crate pinned to Rust `1.82.0`
- [x] Implemented `/health`, `/ready`, request-id middleware, structured request/response logs, and standard error envelope with `X-Request-Id`
- [x] Added engine TCP readiness client that pings `cex-server` with framed PRD §12 JSON
- [x] Implemented auth signup, login, and refresh rotation with argon2id password hashing, HS256 access JWTs, opaque 32-byte base64url refresh tokens, SHA-256 refresh-token storage, and refresh reuse family revocation
- [x] Added SQLx Postgres auth repository for `users`, `refresh_tokens`, and zero-balance seeding from active assets
- [x] Added in-memory auth repository and focused route tests for request IDs, weak password envelope, signup, and refresh
- [x] Root `pnpm check` now includes `cargo check --manifest-path services/Cargo.toml --workspace`

## Next

## Settlement + Market Data (Day 5)

- [x] Added `services/settlement` Rust crate and worker binary
- [x] Added Postgres `LISTEN/NOTIFY` support for `engine_events` inserts via forward-only migration `0003_notify_engine_events.sql`
- [x] Settlement worker consumes unprocessed `engine_events` with `SELECT ... FOR UPDATE SKIP LOCKED` inside the same transaction as settlement writes
- [x] Implemented atomic handling for `orderAccepted`, `fill`, and `orderCanceled` events across `orders`, `trades`, `balances`, `ledger_entries`, and `worker_state`
- [x] Added deterministic settlement tests, including a 1000-fill balance/ledger invariant simulation
- [x] Added `services/market-data` Rust crate and worker binary
- [x] Added Binance combined `bookTicker` URL construction and payload parser for configured active markets
- [x] Added deterministic 10x10 market-maker quote generation around external mid price
- [x] Wired live Binance WebSocket transport for default top-50 USDT stream symbols with reconnect/backoff loop
- [x] Added `klines` persistence table and Binance 1m kline upsert path
- [x] Added demo market-maker/fees users and one-time MM balance bootstrap migration
- [x] Connected market-maker quote refresh to engine cancel/replace order flow
- [x] Added settlement-side engine event bridge that persists engine broadcasts into `engine_events`
- [x] Full Day 5 smoke test passed locally: API ready, Binance klines persisted, MM quotes appeared in engine, engine events bridged to Postgres, and settlement processed all bridged rows

## Security & Rate Limits (Day 7)

- [x] Added `redis = "0.27"` (with `tokio-comp` + `connection-manager`), `hmac = "0.12"`, `totp-rs = "5"` (with `qr`), and `utoipa = "4"` (with `axum_extras`) to workspace dependencies
- [x] Extended `Config` with `redis_url`, `pgcrypto_key`, `faucet_enabled`; `AppState` with `redis: Option<ConnectionManager>`, `pgcrypto_key: Arc<str>`, `faucet_enabled`; builder pattern via `with_redis/pgcrypto_key/faucet_enabled`
- [x] Redis sliding-window rate limiter (`rate_limit.rs`) using Lua script for atomic ZREMRANGEBYSCORE+ZADD; rate limits per PRD §FR-API-03: 10/15min auth, 60/min order write, 600/min read (IP fallback when unauth); rate-limit headers in all responses; fail-open on Redis error
- [x] Security response headers middleware (`security_headers.rs`): HSTS, CSP, X-Content-Type-Options, Referrer-Policy, Permissions-Policy, COOP, CORP, X-Frame-Options per PRD §18.3
- [x] HMAC-SHA256 signature verification middleware (`extractors/hmac.rs`): reads X-AETHER-KEY/TS/SIGN, verifies ±5s timestamp window, looks up + pgp_sym_decrypts API key secret, constant-time HMAC comparison, Redis replay deduplication; injects `HmacCaller` extension on success
- [x] `ApiCaller` extractor (`extractors/api_caller.rs`): tries JWT first (if Authorization: Bearer present), falls back to `HmacCaller` extension; used by order handlers for dual JWT/HMAC auth
- [x] API key CRUD (`handlers/api_keys.rs`): POST creates 32-byte OsRng secret, stores via `pgp_sym_encrypt`, returns secret once; GET lists active keys; DELETE revokes by setting `revoked_at`; full validation (label length, permission allowlist, withdraw disabled)
- [x] TOTP 2FA full flow (`handlers/totp.rs`): `POST /auth/2fa/setup` generates 20-byte OsRng secret, builds TOTP instance, returns otpauth URI + base64 PNG QR code + base32 secret, stores encrypted; `POST /auth/2fa/verify` checks TOTP code, enables flag, returns 8 random backup codes; `POST /auth/2fa/disable` requires current TOTP code + password, clears flag and secret
- [x] `GET /openapi.json` endpoint via `utoipa::OpenApi` derive — returns valid OpenAPI 3.1 document with info, tags, servers, and schema stubs; full handler annotation deferred (see clarifications.md)
- [x] Added repository functions: `find_api_key`, `decrypt_api_key_secret`, `create_api_key`, `revoke_api_key`, `list_api_keys`, `decrypt_totp_secret`, `store_totp_secret`, `enable_totp`, `disable_totp`, `get_user_for_auth`
- [x] Routes wired: `/auth/2fa/{setup,verify,disable}`, `/account/api-keys`, `/account/api-keys/{id}`, `/openapi.json`; middleware layers: security_headers + request_id on all, hmac_auth + rate_limit on `/api/v1`
- [x] 5/5 existing router tests pass; 0 clippy errors; all warnings are pedantic/nursery level (warn, not deny)
- [x] Documented 3 PRD clarifications: API key secret storage (pgcrypto vs. argon2id impossibility), backup codes deferred, OpenAPI annotation deferred

## Frontend Shell (Day 8)

- [x] New deps: `@tanstack/react-query`, `zustand`, `react-hook-form`, `zod`, `@radix-ui/*`, `lucide-react`, `next-themes`
- [x] `globals.css`: dark/light design tokens, custom scrollbar, Radix animation keyframes
- [x] `providers.tsx`: `QueryClientProvider` + `ThemeProvider` + `TokenSync` (rehydrates api-client token from sessionStorage on mount); `ReactQueryDevtools` in dev
- [x] `api-client.ts`: typed fetch with Bearer injection, auto-refresh on 401, structured `ApiError`
- [x] `stores/auth.store.ts`: zustand + sessionStorage persist; `setAuth` / `clearAuth`
- [x] `ws-client.ts`: `useWebSocket` hook — exponential backoff reconnect, channel subscription, sequence-gap re-snapshot trigger
- [x] UI primitives: `Button` (4 variants × 3 sizes), `Input` (labeled, error, hint, aria-compliant), `Skeleton`, `AsyncBoundary` (PRD §17.7 four-state)
- [x] `TopBar`: logo, `MarketSelector` (searchable dropdown, price/pct), 24h stats, nav, account dropdown (with logout)
- [x] Auth route handlers: `/api/auth/{login,signup,refresh,logout}` proxy backend → set httpOnly `aether_refresh` cookie
- [x] Auth pages: login, signup, 2fa (Suspense-wrapped), forgot (mocked send)
- [x] `proxy.ts` (Next.js 16): guards `/trade`, `/portfolio`, `/wallet`, `/account`, `/admin` via `aether_refresh` cookie
- [x] Updated landing page with hero, feature cards, quick-trade links
- [x] 12/12 unit tests green; `pnpm lint`, `pnpm typecheck`, `pnpm build` all pass
- [x] Clarification documented: `@hookform/resolvers@5` / `zod/v4/core` Turbopack incompatibility → inline `zodResolver`

## Trading Screen Part 1 (Day 9)

- [x] Added `lightweight-charts@4.2` per PRD §5.2/Day 9 for production-grade candlestick rendering
- [x] Added `/trade/[symbol]` App Router page backed by a client `TradingScreen`
- [x] Added REST market-data helpers for `/markets`, `/markets/:symbol/orderbook`, `/trades`, and `/klines`
- [x] Fixed frontend WebSocket client protocol to match the Rust hub (`method`, `params.channels`, `afterSeq`) and dispatch `book.<symbol>.snapshot` into the subscribed diff stream
- [x] Order book component loads an engine snapshot, subscribes to `book.<symbol>.diff`, merges deltas deterministically, detects stale diffs, computes totals, and renders virtualized bid/ask rows
- [x] Recent trades component loads REST history, subscribes to `trade.<symbol>`, deduplicates live fills, and displays UTC timestamps
- [x] Chart component loads 1m klines, renders candlesticks via `lightweight-charts`, and updates the active candle from live trade events
- [x] Trading screen reuses the Day 8 `TopBar`/market selector and shares a single WebSocket connection across Day 9 panels
- [x] Added focused unit coverage for order-book sorting, delta merge, stale-delta rejection, and decimal-string comparison
- [x] Moved `themeColor` from metadata to `viewport` for a clean Next.js 16 production build
- [x] `pnpm --filter @aether/web lint`, `typecheck`, `test`, and `build` all pass

## Trading Screen Part 2 (Day 10)

- [x] Added typed trading API helpers for order placement, order listing, cancel, and cancel-all
- [x] Added order form covering all PRD §FR-TRADE-05 order types: limit, market, IOC, FOK, post-only, stop-limit, stop-market, and OCO
- [x] Added client-side order validation for required price, stop price, base quantity, quote quantity, and iceberg display quantity
- [x] Added authenticated activity tabs for open orders, closed order history, and live private fills
- [x] Added open-order cancel actions with query invalidation after mutations
- [x] Added trading hotkeys for buy/sell side selection, order form focus, and submit
- [x] Added focused unit coverage for order payload shaping and hotkey filtering

## Portfolio + Wallet + Account (Day 11)

- [x] Added protected `/portfolio`, `/wallet`, and `/account` App Router pages with the existing dense dark TopBar/layout style
- [x] Added typed account API helpers for profile, balances, ledger history, faucet, TOTP setup/verify/disable, and API key CRUD
- [x] Portfolio page now shows estimated USDT value, priced/held/locked asset counts, balances, and ledger history
- [x] Wallet page now supports demo faucet credits, wallet balances, ledger activity, and local withdrawal address/minimum validation
- [x] Withdrawal submit is intentionally disabled until the backend `/wallet/withdraw` flow is implemented
- [x] Account page now supports profile display, 2FA setup/verify/disable UI, backup-code display, API key creation with one-time secret reveal, and revoke actions
- [x] Added focused account-model tests for portfolio valuation, faucet validation, withdrawal validation, and decimal display
- [x] `pnpm --filter @aether/web lint`, `typecheck`, `test`, and `build` all pass
- [x] Local route smoke test passed for `/portfolio`, `/wallet`, and `/account` on desktop/mobile viewports with a seeded session cookie

## Differentiators (Day 12 Part 1)

- [x] Added proof-of-reserves backend snapshot generation from current non-MM customer liabilities, deterministic SHA-256 Merkle tree construction, root persistence in `proof_of_reserves`, public latest-root endpoint, and authenticated per-user proof endpoint
- [x] Added FIFO account P&L computation for USDT-quoted assets via `GET /account/pnl`
- [x] Added public `/proof-of-reserves` page with root/liability summary and verify-my-balance proof panel
- [x] Added FIFO P&L table to `/portfolio`
- [x] Added Rust unit coverage for Merkle determinism, proof verification, liability summaries, and FIFO P&L; added frontend formatting coverage for signed P&L display
- [x] Verification passed: `cargo check --manifest-path services/Cargo.toml --workspace`, `cargo clippy --manifest-path services/Cargo.toml -p cex-api --all-targets -- -D warnings`, `cargo test --manifest-path services/Cargo.toml -p cex-api`, `pnpm --filter @aether/web lint`, `typecheck`, `test`, and `build`

## Differentiators (Day 12 Part 2)

- [x] Extended `GET /markets/:symbol/trades` with optional replay-window params: `from`, `to`, and `order=asc|desc`, while preserving default newest-first behavior
- [x] Added typed `getReplayTrades` frontend helper for one-minute replay windows
- [x] Added `/markets/[symbol]/replay` page with playback controls, speed controls (`1x`, `4x`, `16x`), position slider, synthetic sandbox book, lightweight-charts replay chart, and trade tape
- [x] Added deterministic replay model tests for query parsing, speed fallback, frame building, book aggregation, and side sorting
- [x] Verification passed: `cargo check --manifest-path services/Cargo.toml -p cex-api`, `cargo clippy --manifest-path services/Cargo.toml -p cex-api --all-targets -- -D warnings`, `cargo test --manifest-path services/Cargo.toml -p cex-api`, `pnpm --filter @aether/web lint`, `typecheck`, `test`, and `build`
- [x] In-app browser smoke test passed for `/markets/BTCUSDT/replay?from=1700000000&speed=4` route shell

## Differentiators (Day 12 Part 3)

- [x] Added engine-client support for market halt/resume commands
- [x] Added admin backend routes: `POST /admin/markets/:symbol/halt`, `POST /admin/markets/:symbol/resume`, `POST /admin/markets/:symbol/cancel-all`, `POST /admin/users/:id/freeze`, and `GET /admin/engine/state`
- [x] Added admin authorization guard using `users.is_admin=true` and active account status
- [x] Added audit-log writes for halt, resume, cancel-all, and freeze-user actions
- [x] Added `/admin` frontend console with market controls, freeze-user form, last-action summary, and engine-state table
- [x] Fixed stale frontend auth paths so protected pages redirect to `/login` and links use `/login`, `/signup`, `/forgot`, and `/2fa`
- [x] Full Day 12 verification passed: `pnpm check`, `pnpm build`, `pnpm e2e`, `cargo test --manifest-path services/Cargo.toml --workspace`, `cargo test --manifest-path engine/Cargo.toml --workspace`, `cargo clippy --manifest-path services/Cargo.toml -p cex-api --all-targets -- -D warnings`, and in-app browser smoke checks for replay/admin routing

## E2E Tests (Day 13 — Batch 4)

- [x] Updated `web/playwright.config.ts`: `globalSetup`, `PLAYWRIGHT_BASE_URL` + `API_BASE_URL` env-var overrides, `maxFailures: 3` in CI, screenshot/video on failure; `webServer` omitted when `PLAYWRIGHT_BASE_URL` is set (for deployed-env runs)
- [x] Updated `web/tsconfig.json`: added `playwright.config.ts` and `e2e/**/*.ts` to include for Node.js type access
- [x] Created `web/e2e/global-setup.ts`: pings `API_BASE/health` before specs; logs clear warning if backend unreachable
- [x] Created `web/e2e/fixtures.ts`: `authedPage` fixture, inline TOTP generator (RFC 6238, SHA-1, base32, no deps), inline HMAC-SHA256 signer matching Rust `extractors/hmac.rs` payload, `getAccessToken` (reads zustand sessionStorage), `signUpAndLogin` helper
- [x] Created `web/e2e/auth.spec.ts` — E2E-01: full signup → 2FA setup (intercepts `/auth/2fa/setup` response for secret) → verify TOTP → logout → login → 2FA challenge → assert `/trade/**`
- [x] Created `web/e2e/orders.spec.ts` — E2E-02: faucet 1000 USDT → limit buy at price 1 → `aria-label="Cancel order"` → assert order gone
- [x] Created `web/e2e/fills.spec.ts` — E2E-03: faucet 5000 USDT → market buy 10 USDT → Fills tab row → portfolio BTC balance (requires market-maker worker; degrades gracefully)
- [x] Created `web/e2e/api-key.spec.ts` — E2E-04: signup via browser → faucet via `request` context → create key via JWT → HMAC-sign `POST /orders` with `X-AETHER-{KEY,TS,SIGN}` headers → assert 201
- [x] Created `web/e2e/replay.spec.ts` — E2E-05: `/markets/BTCUSDT/replay?from=<1h_ago>&speed=4` → Play → Pause visible → slider advances; speed 1x/4x/16x buttons render; graceful degradation when no frames
- [x] Verification: `pnpm typecheck` clean (0 errors), `pnpm test` 32/32 passing

## Deployment Configs (Day 13 — Batch 3)

- [x] Added multi-stage Dockerfiles for all four services using `rust:1.82-slim`/`rust:1.95-slim` → `debian:bookworm-slim`; services Dockerfiles copy both `engine/` and `services/` (path dep on `cex-proto`)
- [x] Added `.dockerignore` at repo root and `engine/.dockerignore` to exclude `target/`, `web/`, `.git` from build contexts
- [x] Added `ENGINE_HOST`/`ENGINE_PORT` env-var reading to `engine/crates/server/src/main.rs`; defaults to loopback for local dev, set to `0.0.0.0` in Fly.io `[env]` section
- [x] Created `infra/fly/{api,engine,settlement,market-data}/fly.toml` for Fly.io Singapore region (`sin`); engine app has persistent volume mount at `/data` for WAL files; API has health-check on `/health`; workers are headless (no `[http_service]`)
- [x] Added `vercel.json` at repo root with pnpm monorepo build/install commands for Vercel Next.js auto-detection
- [x] Added `.env.production.example` documenting `DATABASE_URL`, `REDIS_URL`, `JWT_SECRET`, `PGCRYPTO_KEY`, `OTLP_ENDPOINT`, `NEXT_PUBLIC_*` variables with format examples
- [x] Updated `Makefile` with `deploy` (all services), `deploy-api/engine/settlement/market-data`, `deploy-vercel`, and `fly-secrets-*` helper targets
- [x] Replaced `infra/fly/README.md` with a complete step-by-step guide: Neon + Upstash setup, `fly apps create`, volume create, secrets, deploy order, Vercel env vars, verification, scaling, rollback
- [x] Verification: `cargo check --manifest-path engine/Cargo.toml --workspace` and `cargo check --manifest-path services/Cargo.toml --workspace --all-targets` both clean

## CI Hardening (Day 13 — Batch 2)

- [x] Added `rust-services` CI job to `.github/workflows/ci.yml` using Rust 1.95.0, running `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace` (all service tests are in-memory — no Postgres service container needed)
- [x] Added `lighthouse` CI job using `treosh/lighthouse-ci-action@v11`; builds the Next.js app then audits `http://localhost:3000/` against PRD §17.6 thresholds
- [x] Created `web/lighthouserc.json` with `lighthouse:no-pwa` preset and `minScore` assertions: performance ≥0.9, accessibility ≥0.95, best-practices ≥0.9, SEO ≥0.95
- [x] `e2e` job gate expanded to depend on `rust`, `rust-services` (in addition to `typescript`) so a services compilation failure blocks E2E runs
- [x] Renamed existing `rust` job to `rust (engine)` for clarity; new job named `rust (services)`

## Observability + Deploy (Day 13 — Batch 1)

- [x] Added `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry`, `opentelemetry-semantic-conventions` to services workspace deps
- [x] Created `services/api/src/telemetry.rs`: `OtelGuard` + `init_telemetry()` — wires `TracerProvider` + `SdkMeterProvider` with OTLP gRPC export and `TraceContextPropagator` for W3C traceparent
- [x] Added `otlp_endpoint: Option<String>` config field (env `OTLP_ENDPOINT`) to all three services
- [x] Replaced bare `tracing_subscriber::fmt()` init in `api/main.rs`, `settlement/main.rs`, and `market-data/main.rs` with `init_telemetry()` — `_otel_guard` held for process lifetime
- [x] Rewritten `middleware.rs`: per-request `tracing::info_span!` linked to W3C `traceparent` from incoming headers via `set_parent()` + `instrument()`; records `http_requests_total` counter and `http_request_duration_seconds` histogram via OTel global meter (PRD §19.2)
- [x] Rewritten `engine_client.rs`: injects W3C `traceparent` into every outbound TCP frame JSON by extracting current tracing span context via `tracing_opentelemetry::OpenTelemetrySpanExt`; added `EngineClientError::Serialization` variant
- [x] `settlement/main.rs`: added `settlement_events_processed_total` counter and `settlement_lag_seq` gauge (queries `engine_events MAX(seq) - worker_state.last_processed_seq`)
- [x] Updated `docker-compose.yml`: added `otelcol` (OTel Collector Contrib 0.109.0), `prometheus` (2.53.0), `grafana` (11.2.0) services with proper healthcheck deps and volume mounts
- [x] Created `infra/observability/otel-collector.yml`: OTLP gRPC/HTTP receivers → batch processor → Jaeger (traces) + Prometheus scrape endpoint (metrics)
- [x] Created `infra/observability/prometheus.yml`: scrapes OTel collector's `:8889` Prometheus endpoint every 15 s
- [x] Created Grafana auto-provisioning: `datasources/prometheus.yml` (uid `aether-prometheus`) + `dashboards/dashboards.yml` (file provider pointing to `/etc/grafana/dashboards`)
- [x] Filled `infra/observability/dashboards/aether.json`: 7 panels covering HTTP RPS, HTTP P50/P95/P99 latency, engine orders/sec, settlement lag, active WS connections, DB pool in use, error rate (PRD §19.4)
- [x] Verification: `cargo clippy --manifest-path services/Cargo.toml --workspace --all-targets -- -D warnings` clean; `cargo test --manifest-path services/Cargo.toml -p cex-api` 10/10 pass

## Trading Endpoints + WebSocket Hub (Day 6)

- [x] Fixed pre-existing cold-build breakage: services `rust-toolchain.toml` updated `1.82.0 → 1.95.0` to match lock file (edition-2024 transitive deps). Engine toolchain unchanged. PRD and AGENTS.md updated.
- [x] Extended engine client to persistent multiplexed TCP connection: single `TcpStream` split read/write, `Mutex<Option<OwnedWriteHalf>>` for writes, `DashMap<Uuid, oneshot::Sender>` for pending responses, auto-reconnect on failure. Eliminates per-request TCP handshake (~40 μs saved per order).
- [x] Added JWT decode to `TokenConfig` (pure in-memory HMAC verify, ~1–2 μs)
- [x] Added `AuthenticatedUser` extractor (`extractors/auth.rs`) — `FromRequestParts` impl, no DB round-trip
- [x] Added all error codes from PRD §10.2 to `ErrorCode` enum
- [x] Added `AppState` fields: `token_config`, `db: PgPool`, `engine: EngineClient`, `hub: Hub`
- [x] Implemented `repositories/orders.rs`: insert, find by ID, find by client_order_id (idempotency), list with cursor pagination, update_status, try_lock_balance, unlock_balance
- [x] Implemented `repositories/markets.rs`: list_markets (with 24h rolling stats from klines), get_market, get_market_assets, recent_trades, klines
- [x] Implemented `repositories/account.rs`: get_profile, list_balances, ledger_history (cursor paginated), apply_faucet, get_faucet_limit
- [x] Implemented all `/orders` endpoints: POST (place with idempotency + balance lock + engine forward), GET/:id, GET (list), DELETE/:id (cancel), DELETE (cancel-all)
- [x] Implemented all `/markets/*` endpoints: GET list, GET :symbol, GET :symbol/orderbook (engine snapshot), GET :symbol/trades, GET :symbol/klines
- [x] Implemented `/account`, `/account/balances`, `/account/history`, `/wallet/faucet`
- [x] Implemented WebSocket hub (`ws/hub.rs`): `DashMap<Box<str>, broadcast::Sender<Arc<Bytes>>>` keyed by channel name, 60-second diff replay buffer, per-symbol sequence, zero-copy `Arc<Bytes>` fan-out
- [x] Implemented `GET /ws` handler: per-connection `mpsc::unbounded_channel` decouples socket write from broadcast receivers; per-subscription forwarder tasks; snapshot+diff protocol; server heartbeat every 20 s
- [x] Added `dashmap = "5"`, `bytes = "1"`, and `axum ws` feature to workspace deps
- [x] All 5 existing router tests pass; zero clippy errors
- [x] Fixed SQL bugs: `"type"` reserved keyword quoted throughout orders.rs; INSERT uses CTE so RETURNING can JOIN markets
- [x] Fixed klines repo: table uses `symbol TEXT` + `opened_at` (not `market_id` + `ts`) — all queries corrected
- [x] Added ticker publisher: `Hub::run_ticker` queries 24h rolling stats from klines every 1 s, publishes to `ticker.<symbol>` and `ticker.all`
- [x] Day 6 smoke test: 30/30 checks passed locally — auth, faucet, markets, orderbook, place/get/cancel orders, idempotency, ledger history, WebSocket subscribe+snapshot
