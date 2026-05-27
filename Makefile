.PHONY: build check dev e2e format lint test typecheck \
        deploy deploy-api deploy-engine deploy-settlement deploy-market-data \
        deploy-vercel fly-secrets-api fly-secrets-engine fly-secrets-workers

# ── Development ───────────────────────────────────────────────────────────────

build:
	pnpm build

check:
	pnpm check

dev:
	pnpm dev

e2e:
	pnpm e2e

format:
	pnpm format

lint:
	pnpm lint

test:
	pnpm test

typecheck:
	pnpm typecheck

# ── Production deploy (Fly.io + Vercel) ──────────────────────────────────────
# Prerequisites:
#   - fly CLI authenticated: fly auth login
#   - vercel CLI authenticated: vercel login
#   - Secrets set via fly-secrets-* targets below
#   - Engine volume created: fly volumes create engine_wal --size 1 --region sin --app aether-engine
#
# Usage:
#   make deploy              — deploy all four Fly.io services
#   make deploy-api          — deploy only the API
#   make deploy-vercel       — deploy the frontend to Vercel

deploy: deploy-engine deploy-api deploy-settlement deploy-market-data
	@echo "All Fly.io services deployed."

deploy-api:
	fly deploy --config infra/fly/api/fly.toml --remote-only

deploy-engine:
	fly deploy --config infra/fly/engine/fly.toml --remote-only

deploy-settlement:
	fly deploy --config infra/fly/settlement/fly.toml --remote-only

deploy-market-data:
	fly deploy --config infra/fly/market-data/fly.toml --remote-only

deploy-vercel:
	vercel --prod

# ── Fly.io secret helpers ────────────────────────────────────────────────────
# Edit the values before running — do NOT commit a file with real secrets.
#
# Usage: make fly-secrets-api DATABASE_URL="..." JWT_SECRET="..." ...

fly-secrets-api:
	@echo "Usage: fly secrets set DATABASE_URL=... REDIS_URL=... JWT_SECRET=... PGCRYPTO_KEY=... --app aether-api"

fly-secrets-engine:
	@echo "No secrets required for the engine (it uses no external services)."

fly-secrets-workers:
	@echo "Usage: fly secrets set DATABASE_URL=... --app aether-settlement"
	@echo "Usage: fly secrets set DATABASE_URL=... --app aether-market-data"
