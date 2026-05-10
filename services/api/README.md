# API Service

## Purpose

Rust Axum gateway for HTTP, WebSocket, auth, validation, rate limiting, and engine communication.

## Run Locally

```bash
cargo run --manifest-path services/Cargo.toml -p cex-api
```

Required environment is listed in `.env.example`. The API binds to `API_HOST:API_PORT`
and checks Postgres plus the matching engine for `/ready`.

## Test

```bash
cargo test --manifest-path services/Cargo.toml --workspace
```

## Spec

See `PRD.md` § 10, § 11, and § 18.
