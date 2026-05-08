# Local Setup

## Purpose

This guide bootstraps Aether for local development.

## Prerequisites

- Node.js `24.15.0`
- pnpm `11.0.8`
- Rust via `engine/rust-toolchain.toml`
- Docker Desktop or a compatible Docker engine

## Install

```bash
pnpm install
cargo fetch --manifest-path engine/Cargo.toml
```

## Run

```bash
make dev
```

See `PRD.md` § 21 for the delivery plan.
