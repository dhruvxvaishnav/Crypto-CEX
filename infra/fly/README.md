# Aether — Production Deployment Guide

This directory holds the Fly.io manifests for the four backend services.
The frontend is deployed separately to Vercel.

## Architecture

```
Vercel (Next.js)
      │ HTTPS
      ▼
aether-api.fly.dev  ──── private network ────►  aether-engine.internal:7878
      │                                                     │
      │ postgres (TLS)                        WAL volume (/data)
      ▼
Neon DB (pooler)    Upstash Redis (TLS)
      ▲
      │ postgres
aether-settlement.fly.dev   (background worker)
aether-market-data.fly.dev  (background worker → engine)
```

All four Fly.io apps run in the **sin** (Singapore) region. Inter-service
communication uses Fly's private WireGuard network (`*.internal` DNS).

---

## Prerequisites

```bash
# Install CLIs
brew install flyctl vercel-cli     # macOS
# or: npm install -g vercel

fly auth login
vercel login
```

---

## 1. External services

### Neon (Postgres)

1. Create a project at <https://neon.tech>.
2. Create a database named `aether`.
3. Copy the **connection-pooler URL** (not the direct URL):
   ```
   postgres://user:password@ep-xxx.pooler.us-east-1.aws.neon.tech/aether?sslmode=require
   ```
4. Run migrations against this URL:
   ```bash
   DATABASE_URL="<neon-pooler-url>" \
     cargo sqlx migrate run --manifest-path services/Cargo.toml
   ```
   Or apply `infra/migrations/*.sql` in order with psql.

### Upstash (Redis)

1. Create a Redis database at <https://upstash.com>.
2. Copy the **TLS URL**:
   ```
   rediss://default:<password>@<host>.upstash.io:6379
   ```

---

## 2. Create Fly.io apps

Run once per app (skips existing apps):

```bash
fly apps create aether-engine    --org personal
fly apps create aether-api       --org personal
fly apps create aether-settlement --org personal
fly apps create aether-market-data --org personal
```

### Create the engine WAL volume

The engine needs a persistent volume so WAL files survive machine restarts.

```bash
fly volumes create engine_wal \
  --size 1 \
  --region sin \
  --app aether-engine
```

---

## 3. Set secrets

### API

```bash
fly secrets set \
  DATABASE_URL="<neon-pooler-url>" \
  REDIS_URL="<upstash-tls-url>" \
  JWT_SECRET="$(openssl rand -base64 32)" \
  PGCRYPTO_KEY="$(openssl rand -base64 24)" \
  OTLP_ENDPOINT="" \
  --app aether-api
```

### Settlement worker

```bash
fly secrets set \
  DATABASE_URL="<neon-pooler-url>" \
  --app aether-settlement
```

### Market-data worker

```bash
fly secrets set \
  DATABASE_URL="<neon-pooler-url>" \
  --app aether-market-data
```

> The engine has no secrets — it uses no external services.

---

## 4. Deploy

Deploy in dependency order: **engine first**, then API, then workers.

```bash
# From the repo root:
make deploy

# Or individually:
make deploy-engine
make deploy-api
make deploy-settlement
make deploy-market-data
```

Each `make deploy-*` call runs `fly deploy --config infra/fly/<service>/fly.toml --remote-only`.

---

## 5. Deploy the frontend (Vercel)

```bash
# First time — creates the Vercel project and links it
vercel

# Subsequent production deploys
make deploy-vercel
```

In the Vercel project settings set these **Environment Variables** for Production:

| Variable | Value |
|---|---|
| `NEXT_PUBLIC_API_BASE_URL` | `https://aether-api.fly.dev/api/v1` |
| `NEXT_PUBLIC_WS_URL` | `wss://aether-api.fly.dev/ws` |

Set **Root Directory** to `web` in the Vercel project settings (or leave blank
if `vercel.json` at repo root is detected).

---

## 6. Verify the deployment

```bash
# API health
curl https://aether-api.fly.dev/health

# API readiness (checks DB + engine connectivity)
curl https://aether-api.fly.dev/ready

# Live logs
fly logs --app aether-api
fly logs --app aether-engine
```

---

## Scaling

All apps use `auto_stop_machines = true` with `min_machines_running = 0`
(free-tier friendly). To pin the API to always-on:

```bash
fly scale count 1 --app aether-api
fly m update --app aether-api --min-machines-running 1
```

---

## Rollback

```bash
fly releases list --app aether-api
fly deploy --image registry.fly.io/aether-api:<version> --app aether-api
```
