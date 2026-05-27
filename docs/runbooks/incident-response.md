# Runbook: Incident Response

**Spec:** PRD §19.5. Last updated: 2026-05-27.

---

## Overview

This runbook coordinates a production incident on Aether. It is intentionally
short: **reduce blast radius first, investigate second**.

An "incident" is any condition where:
- Users cannot place or cancel orders for > 2 minutes, OR
- Account balances are incorrect or inaccessible, OR
- The API returns 5xx for > 10% of requests over a 60-second window, OR
- Settlement lag exceeds 1 000 events (PRD §6 backpressure threshold).

---

## Severity levels

| Sev | Condition | Response time |
|---|---|---|
| P1 | Balance corruption or incorrect fills | Immediate — halt all markets |
| P2 | Order intake down but balances intact | < 5 min — halt affected markets |
| P3 | Latency degraded, no data loss | < 15 min — investigate without halting |
| P4 | Cosmetic / non-critical UI issue | Next working day |

---

## Step 1 — Detect and assess (< 2 min)

```bash
# API health
curl https://aether-api.fly.dev/health
curl https://aether-api.fly.dev/ready

# Settlement lag
fly ssh console -a aether-api -- \
  psql "$DATABASE_URL" -c "
    SELECT COALESCE(MAX(e.seq),0)
           - COALESCE((SELECT last_processed_seq FROM worker_state
                       WHERE worker_name='settlement'), 0) AS lag
    FROM engine_events e;"

# Error rate (last 5 min) — check Grafana → Aether Overview → Error Rate
# OR via API logs:
fly logs --app aether-api | grep '"level":"error"' | tail -20

# Engine state
curl https://aether-api.fly.dev/api/v1/admin/engine/state \
  -H "Authorization: Bearer <admin_jwt>"
```

---

## Step 2 — Contain (P1/P2: do immediately)

### Halt all markets

```bash
# Halt order intake for each active market.
for SYMBOL in BTCUSDT ETHUSDT SOLUSDT; do
  curl -X POST "https://aether-api.fly.dev/api/v1/admin/markets/${SYMBOL}/halt" \
    -H "Authorization: Bearer <admin_jwt>"
done
```

A halted market rejects new orders with `MARKET_HALTED` but leaves resting
orders on the book and allows cancellations.

### Cancel all orders (only if book integrity is suspect)

Only do this if you believe the engine book state is corrupted:

```bash
for SYMBOL in BTCUSDT ETHUSDT SOLUSDT; do
  curl -X POST "https://aether-api.fly.dev/api/v1/admin/markets/${SYMBOL}/cancel-all" \
    -H "Authorization: Bearer <admin_jwt>"
done
```

---

## Step 3 — Investigate

Use the relevant specialist runbooks:

| Symptom | Runbook |
|---|---|
| Engine not responding | `docs/runbooks/engine-restart.md` |
| Settlement lag growing | `docs/runbooks/settlement-stuck.md` |
| API 5xx after a schema change | `docs/runbooks/db-migration.md` |
| Fly machine OOM / crash | `fly logs --app <app>` → check memory metrics in Grafana |
| Neon DB unavailable | Check https://status.neon.tech — wait or switch to backup |

```bash
# Fly logs for all services
fly logs --app aether-api      | tail -200
fly logs --app aether-engine   | tail -200
fly logs --app aether-settlement | tail -200

# Trace a specific requestId (from X-Request-Id header in the error response)
fly logs --app aether-api | grep "<requestId>"
```

---

## Step 4 — Resolve and resume

After identifying and fixing the root cause:

1. Run the ledger invariant check (see `docs/runbooks/settlement-stuck.md`).
2. Verify `/ready` returns `true`.
3. Resume markets in reverse order of impact:

```bash
for SYMBOL in BTCUSDT ETHUSDT SOLUSDT; do
  curl -X POST "https://aether-api.fly.dev/api/v1/admin/markets/${SYMBOL}/resume" \
    -H "Authorization: Bearer <admin_jwt>"
done
```

4. Monitor Grafana for 10 minutes:
   - HTTP RPS and error rate back to baseline.
   - Settlement lag at 0.
   - No new error-level log lines.

---

## Step 5 — Post-incident write-up

Within 24 hours, add an entry to `docs/journal.md`:

```markdown
## YYYY-MM-DD — Incident: <one-line description>

**Severity:** P1/P2/P3
**Duration:** HH:MM
**Impact:** <user-facing description>

### Timeline
- HH:MM — first alert / detection
- HH:MM — markets halted
- HH:MM — root cause identified
- HH:MM — fix applied
- HH:MM — markets resumed

### Root cause
<Technical description>

### Fix
<What was changed>

### Action items
- [ ] <preventive measure 1>
- [ ] <preventive measure 2>
```

---

## Emergency contacts

| Role | Contact |
|---|---|
| Owner | @Mr. Stark — email on file |
| Fly.io support | https://community.fly.io |
| Neon support | https://neon.tech/docs/introduction/support |

---

## Related

- `docs/runbooks/engine-restart.md`
- `docs/runbooks/settlement-stuck.md`
- `docs/runbooks/db-migration.md`
- PRD §19 — observability and operations requirements
