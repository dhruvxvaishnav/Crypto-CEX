# Runbook: Engine Restart

**Spec:** PRD §14.6 (WAL), §19 (Observability). Last updated: 2026-05-27.

---

## Overview

The matching engine (`cex-server`, Fly.io app `aether-engine`) is a stateful,
in-memory order book process. It persists all commands to a write-ahead log
(WAL) and replays it on startup to rebuild book state deterministically. A
restart is therefore safe as long as the WAL files on the mounted volume are
intact.

**When to use this runbook:**
- Engine process has crashed or is OOM-killed.
- `/ready` endpoint returns unhealthy for > 60 seconds.
- You need to apply an engine binary update.
- Scheduled maintenance (scheduled order-intake pause).

---

## Pre-restart checks (< 5 min)

```bash
# 1. Check API readiness — engine ping is embedded in the check.
curl https://aether-api.fly.dev/ready

# 2. Check settlement lag (should be near 0 in steady state).
#    If > 500, settlement is falling behind — pause intake before restart.
fly ssh console -a aether-api -- \
  psql "$DATABASE_URL" -c \
  "SELECT COALESCE(MAX(seq),0) - COALESCE(
     (SELECT last_processed_seq FROM worker_state WHERE worker_name='settlement'), 0
   ) AS lag FROM engine_events;"

# 3. Note the current engine sequence (for post-restart verification).
curl https://aether-api.fly.dev/api/v1/admin/engine/state
```

If the settlement lag is **> 500 events**, halt order intake before restarting:

```bash
curl -X POST https://aether-api.fly.dev/api/v1/admin/markets/BTCUSDT/halt \
  -H "Authorization: Bearer <admin_jwt>"
# Repeat for each active market listed in /api/v1/markets.
```

---

## Restart procedure

```bash
# Option A: Fly.io rolling restart (zero-downtime if only one machine).
fly machine restart --app aether-engine

# Option B: Full redeploy (use after a binary update).
make deploy-engine
```

---

## Post-restart verification (< 3 min)

```bash
# 1. Engine health — wait for TCP accept.
#    From the API machine (private network):
fly ssh console -a aether-api -- \
  nc -zv aether-engine.internal 7878

# 2. API readiness.
curl https://aether-api.fly.dev/ready
# Expected: {"ready":true}

# 3. Engine state endpoint.
curl https://aether-api.fly.dev/api/v1/admin/engine/state
# Verify markets are in the expected state (active/halted).

# 4. Confirm settlement lag is draining.
fly logs --app aether-settlement | grep "settlement.batch.processed"

# 5. Place a test order to verify end-to-end flow.
curl -X POST https://aether-api.fly.dev/api/v1/orders \
  -H "Authorization: Bearer <test_jwt>" \
  -H "Content-Type: application/json" \
  -d '{"symbol":"BTCUSDT","side":"buy","type":"limit","price":"1","quantity":"0.001"}'
# Expected: 201 Created
```

---

## Resume order intake (if halted)

```bash
curl -X POST https://aether-api.fly.dev/api/v1/admin/markets/BTCUSDT/resume \
  -H "Authorization: Bearer <admin_jwt>"
```

---

## Escalation

If WAL replay fails (engine exits immediately after start):
1. Check `fly logs --app aether-engine` for the replay error.
2. If the WAL is corrupt, the last resort is to clear it — **all resting orders
   will be lost** and users must re-enter them.
   ```bash
   fly ssh console -a aether-engine
   rm /data/engine.wal /data/engine-events.wal
   exit
   fly machine restart --app aether-engine
   ```
3. Notify users via the status page before clearing the WAL.
4. Write a post-mortem in `docs/journal.md`.

---

## Related

- `docs/runbooks/incident-response.md` — coordination template
- `docs/runbooks/settlement-stuck.md` — if lag does not drain after restart
- PRD §14.6 — WAL replay specification
