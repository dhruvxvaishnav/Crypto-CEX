# Aether

Aether is a production-architecture spot crypto exchange portfolio demonstration. It implements a real Rust matching engine, real order and ledger flows, simulated custody surfaces, and a dense professional trading UI.

This is not a real exchange and never custodies real funds.

## Scope

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

## Repository Layout

```text
engine/                  Rust workspace for matching engine protocol and server crates
packages/shared/         Shared TypeScript primitives and public error codes
web/                     Next.js App Router frontend
services/                Worker/service homes for API, settlement, and market data
infra/                   Compose, migrations, deploy, and observability assets
scripts/                 Local developer entrypoints
docs/                    ADRs, runbooks, journal, clarifications, and v2 ideas
```

## Local Development

Install Node.js `24.15.0`, pnpm `11.0.8`, and Rust via `engine/rust-toolchain.toml`.

```bash
pnpm install
make dev
```

## Verification

```bash
pnpm lint
pnpm typecheck
pnpm test
cargo check --manifest-path engine/Cargo.toml --workspace
```

## Specification

The product contract lives in `PRD.md`. Engineering rules live in `AGENTS.md`.
