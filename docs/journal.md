# Engineering Journal

## 2026-05-27 — Day 13: Observability + Deploy

Goal: wire OpenTelemetry across all three Rust services, get the full
observability stack (collector → Prometheus → Grafana) running in docker-compose,
harden CI with a services Rust job and Lighthouse thresholds, write all
deployment configs (Dockerfiles, fly.toml, vercel.json), and deliver all 5
PRD E2E scenarios.

**Outcome:** All four batches shipped.

- Batch 1: OTel SDK in api/settlement/market-data; Grafana dashboard with 7 panels;
  `settlement_lag_seq` gauge live; engine TCP frames carry W3C `traceparent`.
- Batch 2: `rust-services` CI job (checks + clippy + nextest); `lighthouse` CI job
  via `treosh/lighthouse-ci-action`; `web/lighthouserc.json` thresholds.
- Batch 3: Multi-stage Dockerfiles for all 4 services; `fly.toml` manifests
  (Singapore region); engine WAL volume; `make deploy` target; `vercel.json`;
  `.env.production.example`; full `infra/fly/README.md` deploy guide.
- Batch 4: 5 Playwright E2E specs (E2E-01 through E2E-05); inline TOTP
  generator + HMAC signer (no deps); `PLAYWRIGHT_BASE_URL` / `API_BASE_URL`
  env-var support for deployed-env smoke testing.
- Batch 5: Runbooks fleshed out with actionable commands, real table names,
  and escalation paths.

## 2026-05-09

Goal: establish the Day 1 foundation so feature work has a stable monorepo, infrastructure, and CI base.
