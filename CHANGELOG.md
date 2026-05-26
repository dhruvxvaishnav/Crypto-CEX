# Changelog

All notable changes to Aether are recorded here.

## [Unreleased]

### Differentiators (Day 12)

- Added proof-of-reserves snapshot generation from live customer liabilities, Merkle root commitment to `proof_of_reserves`, public latest-root API, and authenticated per-user proof API.
- Added `/proof-of-reserves` with liabilities summary, root display, and verify-my-balance panel with leaf payloads and sibling hashes.
- Added FIFO portfolio P&L via `GET /account/pnl` and surfaced unrealised/realised P&L on the portfolio page.
- Added replay-mode trade history support via `from`/`to`/`order=asc` query params and a `/markets/[symbol]/replay` page with playback controls, synthetic sandbox book, replay chart, and trade tape.

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
