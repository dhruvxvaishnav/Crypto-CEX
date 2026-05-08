.PHONY: build check dev e2e format lint test typecheck

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
