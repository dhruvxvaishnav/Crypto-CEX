# ── Stage 1: Build ───────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY engine/ engine/
COPY services/ services/

WORKDIR /app/services
RUN cargo build --release --bin cex-settlement

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/services/target/release/cex-settlement /usr/local/bin/cex-settlement

ENTRYPOINT ["cex-settlement"]
