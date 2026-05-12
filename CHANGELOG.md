# Changelog

All notable changes to Aether are recorded here.

## [Unreleased]

### Tooling

- Approved the frontend stack refresh to latest stable Next.js, React, Tailwind, Biome, TypeScript, and pnpm versions as of 2026-05-09.

### Foundations

- Added the initial monorepo foundation for Rust engine crates, the Next.js web app, shared TypeScript package, infrastructure, scripts, and CI.

### Engine

- Added Day 4A engine TCP server and protocol spine: framed JSON wire types, loopback listener, WAL-backed command routing, durable sequenced event broadcast, snapshots, and TCP integration tests.

### API

- Added Day 4B API skeleton and auth slice: Axum service crate, health/readiness routes, request IDs, error envelope, structured logs, engine readiness client, signup/login/refresh auth, argon2id password hashing, JWT access tokens, and refresh-token rotation.
- Added Day 7 security and rate-limit layer: Redis sliding-window rate limiter per PRD §FR-API-03 (IP + user tiers), HMAC-SHA256 signature verification middleware (PRD §18.4) with replay protection, API key CRUD endpoints (`POST/GET /account/api-keys`, `DELETE /account/api-keys/:id`) with `pgp_sym_encrypt` secret storage, TOTP 2FA full flow (`/auth/2fa/setup|verify|disable`), all PRD §18.3 security response headers, and `GET /openapi.json` via `utoipa`.
