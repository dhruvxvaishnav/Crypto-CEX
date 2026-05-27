# ── Stage 1: Build ───────────────────────────────────────────────────────────
# Pinned to the services toolchain (AGENTS.md §4.1).
# Build context = repo root (services/ references engine/crates/proto as path dep).
FROM rust:1.95-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy engine crates first (path dep for cex-proto).
COPY engine/ engine/
COPY services/ services/

WORKDIR /app/services
RUN cargo build --release --bin cex-api

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# ca-certificates required for TLS connections to Neon, Upstash, and OTLP collectors.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/services/target/release/cex-api /usr/local/bin/cex-api

EXPOSE 8080

ENTRYPOINT ["cex-api"]
