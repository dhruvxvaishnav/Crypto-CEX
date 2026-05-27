# Runbook: Database Migration

**Spec:** AGENTS.md §6 (SQL & Migrations), PRD §9 (Data Model).
Last updated: 2026-05-27.

---

## Overview

All schema changes are applied via **forward-only, sequentially-numbered SQL
files** in `infra/migrations/`. Every file is applied exactly once. To undo a
migration, a new file that reverses the change must be created — never edit an
already-applied file.

---

## Before writing a migration

1. **Read AGENTS.md §6** in full.
2. Check that the next migration number is the highest existing one + 1:
   ```bash
   ls infra/migrations/ | sort | tail -5
   ```
3. Estimate lock impact:
   - `CREATE INDEX CONCURRENTLY` — no table lock, safe on live data.
   - `ALTER TABLE … ADD COLUMN … NOT NULL DEFAULT …` — full table rewrite on
     Postgres < 11; on Postgres 16 (Neon) adding a column with a constant
     default is near-instant (metadata-only).
   - `ALTER TABLE … DROP COLUMN` — safe but may block briefly.
   - `CREATE TYPE … AS ENUM` — safe.
   - Adding a `NOT NULL` constraint without a default **locks the table for a
     full scan** — use a two-step migration (add nullable, backfill, add constraint).

4. For migrations expected to run > 5 s on a table with > 100 k rows, add a
   comment at the top of the file:
   ```sql
   -- LOCK IMPACT: estimated 10–30 s on ledger_entries (currently ~2 M rows).
   -- Schedule during off-peak hours and halt markets before applying.
   ```

---

## Applying a migration

### Local development

```bash
# Ensure docker-compose services are running.
make dev   # or: docker-compose -f infra/compose/docker-compose.yml up -d

# Apply via sqlx CLI (reads DATABASE_URL from .env).
sqlx migrate run --source infra/migrations

# Verify
sqlx migrate info --source infra/migrations
```

### Neon production

```bash
# Use the direct (non-pooler) Neon URL for DDL — pgBouncer in transaction mode
# does not support multi-statement migrations.
export DATABASE_URL="postgres://user:pass@ep-xxx.us-east-1.aws.neon.tech/aether?sslmode=require"

# 1. Take a snapshot/backup on the Neon console before any destructive DDL.

# 2. Check active connections and expected downtime.
psql "$DATABASE_URL" -c \
  "SELECT COUNT(*) AS connections FROM pg_stat_activity WHERE state != 'idle';"

# 3. For long-running migrations, halt order intake first.
curl -X POST https://aether-api.fly.dev/api/v1/admin/markets/BTCUSDT/halt \
  -H "Authorization: Bearer <admin_jwt>"

# 4. Apply the migration.
sqlx migrate run --source infra/migrations --database-url "$DATABASE_URL"

# 5. Verify all migrations show "Applied".
sqlx migrate info --source infra/migrations --database-url "$DATABASE_URL"

# 6. Run smoke tests against production.
curl https://aether-api.fly.dev/ready
curl https://aether-api.fly.dev/api/v1/markets

# 7. Resume markets (if halted).
curl -X POST https://aether-api.fly.dev/api/v1/admin/markets/BTCUSDT/resume \
  -H "Authorization: Bearer <admin_jwt>"
```

---

## Rollback strategy

Because migrations are forward-only, rollback means:

1. **Write a new migration** that reverses the change.
2. Apply it immediately following the same procedure above.
3. If data has been inserted that depends on the new schema, the rollback
   migration must handle it explicitly (backfill, convert, or delete).

Example rollback migration name: `0006_revert_add_fee_tier_column.sql`.

---

## Naming conventions (AGENTS.md §6.2)

| Object | Convention | Example |
|---|---|---|
| Tables | plural `snake_case` | `ledger_entries` |
| Columns | `snake_case`, no abbreviations | `user_id`, `engine_seq` |
| Foreign keys | `<table_singular>_id` | `order_id` |
| Indexes | `idx_<table>_<columns>` | `idx_orders_user_id_created_at` |
| Unique indexes | `uq_<table>_<columns>` | `uq_orders_client_order_id` |
| ENUM types | `<domain>_status` | `order_status` |

---

## Post-migration checklist

```
[ ] sqlx migrate info shows all files as "Applied" with no version skipped
[ ] Application smoke: /ready returns {"ready":true}
[ ] Key API endpoints return expected data shape
[ ] Grafana: DB pool gauge is stable (no connection exhaustion)
[ ] CHANGELOG.md updated under [Unreleased]
[ ] docs/journal.md entry for the migration written
```

---

## Related

- `AGENTS.md §6` — full SQL and migration rules
- `infra/migrations/` — migration source files
- `docs/runbooks/incident-response.md` — if the migration causes unexpected data issues
