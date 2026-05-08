# Status

Accepted.

# Context

The original PRD pinned the frontend to Next.js 15 and Tailwind 3.4. The owner requested the latest stable frontend stack before foundation work continued.

# Decision

Pin the frontend to the latest stable versions verified on 2026-05-09: Node `24.15.0` LTS, pnpm `11.0.8`, Next.js `16.2.6`, React/React DOM `19.2.6`, Tailwind `4.3.0`, TypeScript `6.0.3`, and Biome `2.4.14`.

# Consequences

Future frontend work must read current Next.js 16 documentation and avoid assumptions from older App Router versions.
