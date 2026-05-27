# ── Stage 1: Build ───────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY engine/ engine/
COPY services/ services/

WORKDIR /app/services
RUN cargo build --release --bin cex-market-data

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# ca-certificates needed for outbound TLS to Binance WebSocket feed.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/services/target/release/cex-market-data /usr/local/bin/cex-market-data

ENTRYPOINT ["cex-market-data"]
