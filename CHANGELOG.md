# Changelog

All notable changes to Aether are recorded here.

## [Unreleased]

### Runbooks (Day 13 — Batch 5)

- Rewrote `docs/runbooks/engine-restart.md` with full pre-restart checklist, WAL-corrupt escalation path, and post-restart order-intake verification commands.
- Rewrote `docs/runbooks/settlement-stuck.md` with lag-diagnosis SQL, three root-cause branches (crash, Neon cold-start, long-running lock), and a post-recovery ledger invariant check.
- Rewrote `docs/runbooks/db-migration.md` with local and Neon production apply procedures, lock-impact estimation guidelines, rollback strategy, and a post-migration checklist.
- Rewrote `docs/runbooks/incident-response.md` with a severity matrix, detect → contain → investigate → resolve → post-mortem template, and cross-links to all specialist runbooks.
- Added Day 13 entry to `docs/journal.md`.

### E2E Tests (Day 13 — Batch 4)

- Added `web/e2e/global-setup.ts`: checks API backend reachability before any spec runs; prints a clear warning when the Rust stack isn't up.
- Added `web/e2e/fixtures.ts`: `authedPage` fixture (signup + login via UI for each test), `generateTotp` (RFC 6238, no deps), `buildHmacSignature` (HMAC-SHA256 matching `extractors/hmac.rs` payload format), `getAccessToken` (reads sessionStorage `aether-auth`), `signUpAndLogin`.
- Added `web/e2e/auth.spec.ts` — **E2E-01**: sign up → start 2FA setup → intercept `secret` from API response → verify TOTP code → log out → log in → complete 2FA challenge → assert on `/trade/**`.
- Added `web/e2e/orders.spec.ts` — **E2E-02**: faucet 1000 USDT → place limit buy → assert open order in activity panel → cancel → assert order gone.
- Added `web/e2e/fills.spec.ts` — **E2E-03**: faucet → market buy → assert fill row in Fills tab → assert BTC balance on portfolio page (requires market-maker worker).
- Added `web/e2e/api-key.spec.ts` — **E2E-04**: signup → faucet via API → create API key → build HMAC-signed `POST /orders` → assert 201 (full round-trip of the Binance-compatible signing scheme).
- Added `web/e2e/replay.spec.ts` — **E2E-05**: open `/markets/BTCUSDT/replay?from=…&speed=4` → Play → assert Pause visible and slider advances; speed buttons 1x/4x/16x all rendered; degrades gracefully when no historical data available.
- Updated `web/playwright.config.ts`: added `globalSetup`, `PLAYWRIGHT_BASE_URL` env-var override, `API_BASE_URL` support, screenshot/video on failure, `maxFailures` in CI mode.
- Updated `web/tsconfig.json`: added `playwright.config.ts` and `e2e/**/*.ts` to `include` so Node.js types are available in E2E files.

### Deployment (Day 13 — Batch 3)

- Added multi-stage Dockerfiles for all four services: `engine/Dockerfile` (Rust 1.82-slim), `services/api/Dockerfile`, `services/settlement/Dockerfile`, `services/market-data/Dockerfile` (all Rust 1.95-slim → debian:bookworm-slim runtime).
- Added `.dockerignore` at repo root (excludes `web/`, `*/target/`, `.git`) and `engine/.dockerignore`.
- Added `infra/fly/api/fly.toml`, `infra/fly/engine/fly.toml`, `infra/fly/settlement/fly.toml`, `infra/fly/market-data/fly.toml` for Fly.io Singapore region (`sin`).
- Engine fly.toml mounts a persistent volume (`engine_wal` at `/data`) for WAL crash-recovery files.
- Added `ENGINE_HOST`/`ENGINE_PORT` env-var support to `engine/crates/server/src/main.rs` so the engine can bind to `0.0.0.0` in containerised deployments without breaking the loopback default for local dev.
- Added `vercel.json` at repo root with pnpm monorepo build commands for Vercel Next.js hosting.
- Added `.env.production.example` documenting all required production secrets and env vars.
- Updated `Makefile` with `deploy`, `deploy-api`, `deploy-engine`, `deploy-settlement`, `deploy-market-data`, and `deploy-vercel` targets.
- Replaced `infra/fly/README.md` placeholder with a complete step-by-step guide: Neon DB setup, Upstash Redis, Fly app creation, volume creation, secrets, deploy order, Vercel frontend, verification, scaling, and rollback.

### CI Hardening (Day 13 — Batch 2)

- Added `rust-services` CI job (Rust 1.95.0) that runs `cargo check`, `cargo clippy -D warnings`, and `cargo nextest run` across the full services workspace on every PR — previously only the engine was CI-tested.
- Added `lighthouse` CI job using `treosh/lighthouse-ci-action@v11` that builds the Next.js app and audits the landing page against PRD §17.6 thresholds (≥90 performance, ≥95 accessibility, ≥90 best-practices, ≥95 SEO).
- Added `web/lighthouserc.json` with `lighthouse:no-pwa` preset and explicit `minScore` assertions for all four categories.
- `e2e` job now depends on both `rust` (engine) and `rust-services` so a services build failure blocks E2E.

### Observability (Day 13 — Batch 1)

- Added OpenTelemetry SDK (`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry`, `opentelemetry-semantic-conventions`) to all three Rust services.
- Added `services/api/src/telemetry.rs`: `OtelGuard` + `init_telemetry()` — wires `TracerProvider` + `SdkMeterProvider` with OTLP gRPC export and W3C `TraceContextPropagator`.
- Added `OTLP_ENDPOINT` env var to all three service configs; falls back to `http://127.0.0.1:4317`.
- API middleware now creates a per-request tracing span linked to upstream `traceparent`, records `http_requests_total` and `http_request_duration_seconds` (PRD §19.2).
- Engine client injects W3C `traceparent` into every outbound TCP frame so engine spans link to the API parent trace (PRD §19.3).
- Settlement worker emits `settlement_lag_seq` gauge (live DB query) and `settlement_events_processed_total` counter.
- Updated `docker-compose.yml` to add OTel Collector Contrib, Prometheus, and Grafana with full auto-provisioning.
- Added `infra/observability/otel-collector.yml`, `prometheus.yml`, and Grafana datasource/dashboard provisioning configs.
- Filled `infra/observability/dashboards/aether.json` with 7 panels covering all PRD §19.4 requirements.

### Differentiators (Day 12)

- Added proof-of-reserves snapshot generation from live customer liabilities, Merkle root commitment to `proof_of_reserves`, public latest-root API, and authenticated per-user proof API.
- Added `/proof-of-reserves` with liabilities summary, root display, and verify-my-balance panel with leaf payloads and sibling hashes.
- Added FIFO portfolio P&L via `GET /account/pnl` and surfaced unrealised/realised P&L on the portfolio page.
- Added replay-mode trade history support via `from`/`to`/`order=asc` query params and a `/markets/[symbol]/replay` page with playback controls, synthetic sandbox book, replay chart, and trade tape.
- Added admin backend routes and `/admin` console for market halt/resume, market cancel-all, user freeze, audit logging, and engine-state inspection.
- Fixed frontend auth navigation/proxy paths so protected routes redirect to `/login` instead of the stale `/auth/login` path.

### Frontend (Day 9)

- Added the `/trade/[symbol]` trading screen with shared live WebSocket state, top-bar market context, order book, chart, and recent-trades panels.
- Added `lightweight-charts@4.2` for PRD Day 9 candlestick rendering and wired kline history plus live trade-driven candle updates.
- Added order-book snapshot loading, WebSocket diff subscription, deterministic delta merge, cumulative totals, and virtualized bid/ask rows.
- Added recent-trades history loading with live `trade.<symbol>` updates, dedupe, and UTC timestamp formatting.
- Updated the frontend WebSocket client to match the Rust hub protocol (`method`, `params.channels`, `afterSeq`) and route `book.<symbol>.snapshot` frames into diff subscribers.
- Added typed market-data helpers and order-book model tests covering sort/merge/stale-delta behavior.
- Moved Next.js `themeColor` config from metadata to `viewport` to keep production builds warning-free on Next.js 16.

### Frontend (Day 8)

- Added `@tanstack/react-query`, `zustand`, `react-hook-form`, `@radix-ui/*`, `lucide-react`, and `next-themes` dependencies for the frontend shell.
- Added design tokens in `globals.css` (dark default / light theme), custom scrollbar, and Radix animation keyframes.
- Added `Providers` client component wrapping `QueryClientProvider` + `ThemeProvider` with `ReactQueryDevtools` in dev.
- Added `useAuthStore` (zustand, sessionStorage-persisted) and `api-client.ts` with token injection, auto-refresh on 401, and typed `ApiError`.
- Added `useWebSocket` hook with exponential backoff reconnect (1–16 s), channel subscription management, re-subscription on reconnect, and sequence-number gap detection triggering re-snapshot.
- Added `Button`, `Input`, `Skeleton`, and `AsyncBoundary` UI primitives (PRD §17.5 a11y-compliant, PRD §17.7 four-state pattern).
- Added `TopBar` with logo, live `MarketSelector` dropdown (search + price change), 24h stats, nav links, and account dropdown menu.
- Added auth route handlers (`/api/auth/{login,signup,refresh,logout}`) that proxy to the backend and set `aether_refresh` as httpOnly, Secure, SameSite=Lax cookie.
- Added auth pages: `login`, `signup`, `2fa`, and `forgot` — all with `react-hook-form` + zod validation, inline server-error mapping, and accessible error regions.
- Declared `zod` as a direct `@aether/web` dependency so clean CI installs can typecheck auth validation imports.
- Added `proxy.ts` (Next.js 16 route guard) protecting `/trade`, `/portfolio`, `/wallet`, `/account`, `/admin` behind the `aether_refresh` cookie presence check.
- Added inline `zodResolver` utility (`web/src/lib/zod-resolver.ts`) to avoid Turbopack incompatibility with `@hookform/resolvers@5 → zod/v4/core` subpath export (see `docs/clarifications.md`).
- Added unit tests: `api-client` (5 cases), `auth.store` (2 cases), `Button` component (5 cases) — 12/12 green.
- Updated landing page to full marketing shell with hero, feature cards, and market quick-links.
- Renamed `src/middleware.ts` → `src/proxy.ts` and export to `proxy` per Next.js 16 file convention.
- `pnpm build` succeeds; `pnpm lint`, `pnpm typecheck`, `pnpm test` all green.

### Tooling

- Approved the frontend stack refresh to latest stable Next.js, React, Tailwind, Biome, TypeScript, and pnpm versions as of 2026-05-09.

### Foundations

- Added the initial monorepo foundation for Rust engine crates, the Next.js web app, shared TypeScript package, infrastructure, scripts, and CI.

### Engine

- Added Day 4A engine TCP server and protocol spine: framed JSON wire types, loopback listener, WAL-backed command routing, durable sequenced event broadcast, snapshots, and TCP integration tests.

### API

- Added Day 4B API skeleton and auth slice: Axum service crate, health/readiness routes, request IDs, error envelope, structured logs, engine readiness client, signup/login/refresh auth, argon2id password hashing, JWT access tokens, and refresh-token rotation.
- Added Day 7 security and rate-limit layer: Redis sliding-window rate limiter per PRD §FR-API-03 (IP + user tiers), HMAC-SHA256 signature verification middleware (PRD §18.4) with replay protection, API key CRUD endpoints (`POST/GET /account/api-keys`, `DELETE /account/api-keys/:id`) with `pgp_sym_encrypt` secret storage, TOTP 2FA full flow (`/auth/2fa/setup|verify|disable`), all PRD §18.3 security response headers, and `GET /openapi.json` via `utoipa`.
