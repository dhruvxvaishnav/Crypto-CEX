# Engine Restart Runbook

## Purpose

Restart the matching engine while preserving deterministic recovery from WAL and snapshots.

## Steps

1. Check API readiness and settlement lag.
2. Stop order intake if settlement lag is above the PRD threshold.
3. Restart the engine process.
4. Verify WAL replay completes and `/ready` is healthy.

Spec: `PRD.md` § 14.6 and § 19.
