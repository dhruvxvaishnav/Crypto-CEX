# ── Stage 1: Build ───────────────────────────────────────────────────────────
# Pinned to the engine toolchain (PRD §4.1; AGENTS.md §4.1).
FROM rust:1.82-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy the full engine workspace (context = engine/).
COPY . .

RUN cargo build --release --bin cex-server

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cex-server /usr/local/bin/cex-server

# WAL files are persisted to /data via a Fly.io volume.
# EngineServerConfig uses relative paths so they resolve to /data/*.wal.
WORKDIR /data

EXPOSE 7878

ENTRYPOINT ["cex-server"]
