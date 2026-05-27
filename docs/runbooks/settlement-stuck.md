# Runbook: Settlement Stuck

**Spec:** PRD §15 (Settlement & Ledger), §19.2 (`settlement_lag_seq` metric).
Last updated: 2026-05-27.

---

## Overview

The settlement worker (`cex-settlement`, Fly.io app `aether-settlement`)
consumes rows from `engine_events` and applies atomic ledger writes. It is
"stuck" when `settlement_lag_seq` (reported via Grafana and the
`settlement_lag_seq` OTel gauge) is growing rather than draining.

**Healthy baseline:** `settlement_lag_seq` ≤ 10 in steady state, returning to 0
within 5 seconds of a burst.

**Alert threshold (PRD §6):** lag > 1000 — new orders should be rejected until
it drains.

---

## Diagnosis

```bash
# 1. Check the current lag.
fly ssh console -a aether-api -- \
  psql "$DATABASE_URL" -c "
    SELECT
      COALESCE(MAX(e.seq), 0) AS max_engine_seq,
      COALESCE(ws.last_processed_seq, 0) AS last_settled,
      COALESCE(MAX(e.seq), 0)
        - COALESCE(ws.last_processed_seq, 0) AS lag_seq
    FROM engine_events e
    CROSS JOIN worker_state ws
    WHERE ws.worker_name = 'settlement';"

# 2. Check settlement logs for errors.
fly logs --app aether-settlement | tail -100

# 3. Check for blocked DB connections or long-running transactions.
fly ssh console -a aether-api -- \
  psql "$DATABASE_URL" -c "
    SELECT pid, state, wait_event_type, wait_event, query_start,
           left(query, 80) AS query
    FROM pg_stat_activity
    WHERE state != 'idle'
    ORDER BY query_start;"

# 4. Check for row-level locks on engine_events.
fly ssh console -a aether-api -- \
  psql "$DATABASE_URL" -c "
    SELECT l.pid, l.granted, l.mode, a.query_start,
           left(a.query, 80) AS query
    FROM pg_locks l
    JOIN pg_stat_activity a ON l.pid = a.pid
    WHERE l.relation = 'engine_events'::regclass;"
```

---

## Common causes and fixes

### A. Settlement worker crashed

```bash
fly status --app aether-settlement
# If status shows "failed" or the machine count is 0:
fly machine restart --app aether-settlement
# Watch logs until "settlement.worker.started" appears.
fly logs --app aether-settlement -f
```

### B. Postgres connectivity issue (Neon)

Neon free-tier databases auto-suspend after 5 minutes of inactivity. The first
request after wakeup has a cold-start penalty of 1–3 seconds.

```bash
# Verify connectivity:
fly ssh console -a aether-settlement -- \
  psql "$DATABASE_URL" -c "SELECT 1;"
# If this hangs, the issue is upstream (Neon status: https://status.neon.tech).
```

If Neon is down, halt order intake to stop the lag growing:

```bash
# Halt all active markets (repeat per symbol).
curl -X POST https://aether-api.fly.dev/api/v1/admin/markets/BTCUSDT/halt \
  -H "Authorization: Bearer <admin_jwt>"
```

### C. Long-running transaction blocking `SELECT FOR UPDATE SKIP LOCKED`

The settlement worker uses `SELECT … FOR UPDATE SKIP LOCKED`. If another
transaction holds a lock on `engine_events` rows for > 60 s, terminate it:

```bash
fly ssh console -a aether-api -- \
  psql "$DATABASE_URL" -c "
    SELECT pg_terminate_backend(pid)
    FROM pg_stat_activity
    WHERE state != 'idle'
      AND query_start < NOW() - INTERVAL '60 seconds'
      AND query ILIKE '%engine_events%';"
```

### D. Large backlog after engine restart

If the engine replayed thousands of events at once, the settlement queue will
spike. This is self-healing — the worker processes at up to ~1000 events/second.
No action needed unless lag exceeds 50 000 (> 50 s to drain).

---

## Verify recovery

```bash
# Lag should decrease toward 0 within 30 seconds of the fix.
watch -n 2 "fly ssh console -a aether-api -- psql \"\$DATABASE_URL\" -c \
  \"SELECT COALESCE(MAX(seq),0)-(SELECT last_processed_seq FROM worker_state \
    WHERE worker_name='settlement') FROM engine_events;\""

# Grafana dashboard: Aether Overview → Settlement Lag panel.
```

---

## Ledger invariant check (after recovery)

Before resuming halted markets, verify the ledger is consistent:

```sql
-- All balance rows should satisfy: available + locked >= 0
SELECT COUNT(*)
FROM balances
WHERE available < 0 OR locked < 0;
-- Expected: 0

-- Every fill should have two symmetric ledger entries (taker + maker)
SELECT trade_seq, COUNT(*) AS entry_count
FROM ledger_entries
GROUP BY trade_seq
HAVING COUNT(*) != 2;
-- Expected: 0 rows
```

---

## Escalation

If the ledger invariant check fails, **do not resume order intake** and open an
incident using `docs/runbooks/incident-response.md`.

---

## Related

- `docs/runbooks/engine-restart.md` — often precedes a lag spike
- `docs/runbooks/incident-response.md`
- PRD §15 — ledger rules and atomic settlement guarantees
