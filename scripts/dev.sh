#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${ROOT_DIR}"

docker compose -f infra/compose/docker-compose.yml up -d postgres redis jaeger
pnpm --filter @aether/web dev
