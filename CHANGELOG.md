# Changelog

All notable changes to Aether are recorded here.

## [Unreleased]

### Tooling

- Approved the frontend stack refresh to latest stable Next.js, React, Tailwind, Biome, TypeScript, and pnpm versions as of 2026-05-09.

### Foundations

- Added the initial monorepo foundation for Rust engine crates, the Next.js web app, shared TypeScript package, infrastructure, scripts, and CI.

### Engine

- Added Day 4A engine TCP server and protocol spine: framed JSON wire types, loopback listener, WAL-backed command routing, durable sequenced event broadcast, snapshots, and TCP integration tests.
