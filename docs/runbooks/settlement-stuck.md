# Settlement Stuck Runbook

## Purpose

Recover a settlement worker that stopped consuming engine events.

## Steps

1. Inspect `settlement_lag_seq`.
2. Check Postgres connectivity and locks on `engine_events`.
3. Restart the settlement worker.
4. Run ledger invariant checks before resuming normal traffic.

Spec: `PRD.md` § 15 and § 19.
