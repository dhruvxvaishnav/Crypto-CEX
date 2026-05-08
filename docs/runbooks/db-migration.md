# DB Migration Runbook

## Purpose

Apply forward-only database migrations safely.

## Steps

1. Review the migration for lock impact.
2. Apply in staging.
3. Run invariant and smoke tests.
4. Apply in production during the agreed window.

Spec: `PRD.md` § 9 and `AGENTS.md` § 6.
