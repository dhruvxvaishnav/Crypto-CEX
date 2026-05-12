#!/usr/bin/env bash
# Day 6 smoke test — validates the full trading REST + WebSocket surface.
# Usage: ./scripts/smoke-day6.sh [API_BASE_URL]
# Default: http://127.0.0.1:8080
set -euo pipefail

API="${1:-http://127.0.0.1:8080}"
PASS=0
FAIL=0

grn() { printf '\033[0;32m✓ %s\033[0m\n' "$*"; ((PASS++)) || true; }
red() { printf '\033[0;31m✗ %s\033[0m\n' "$*"; ((FAIL++)) || true; }
hdr() { printf '\n\033[1m== %s ==\033[0m\n' "$*"; }

require_cmd() { command -v "$1" &>/dev/null || { echo "ERROR: $1 not found"; exit 1; }; }
require_cmd curl
require_cmd jq

# ── helpers ──────────────────────────────────────────────────────────────────

api() {
  local method="$1" path="$2"; shift 2
  curl -sf -X "$method" "$API/api/v1$path" \
    -H "Content-Type: application/json" \
    "$@"
}

api_auth() {
  local method="$1" path="$2" token="$3"; shift 3
  curl -sf -X "$method" "$API/api/v1$path" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $token" \
    "$@"
}

check_field() {
  local label="$1" json="$2" field="$3" expected="$4"
  local actual
  actual=$(echo "$json" | jq -r "$field" 2>/dev/null || echo "PARSE_ERROR")
  if [ "$actual" = "$expected" ]; then
    grn "$label ($field = $expected)"
  else
    red "$label — expected $field=$expected got $actual"
  fi
}

check_present() {
  local label="$1" json="$2" field="$3"
  local actual
  actual=$(echo "$json" | jq -r "$field" 2>/dev/null || echo "null")
  if [ "$actual" != "null" ] && [ "$actual" != "" ]; then
    grn "$label ($field present)"
  else
    red "$label — $field missing or null in: $(echo "$json" | head -c 200)"
  fi
}

# ── 1. Liveness ───────────────────────────────────────────────────────────────

hdr "Liveness & readiness"
HEALTH=$(curl -sf "$API/health" || echo '{}')
check_field "health" "$HEALTH" '.status' 'ok'

# ── 2. Auth ───────────────────────────────────────────────────────────────────

hdr "Auth — signup + login"
EMAIL="smoke-$(date +%s)@example.com"
PASSWORD="Sm0ke!Test1ng"

SIGNUP=$(api POST /auth/signup -d "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
check_present "signup accessToken"  "$SIGNUP" '.accessToken'
check_present "signup refreshToken" "$SIGNUP" '.refreshToken'
TOKEN=$(echo "$SIGNUP" | jq -r '.accessToken')

LOGIN=$(api POST /auth/login -d "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
check_present "login accessToken" "$LOGIN" '.accessToken'

# ── 3. Account — profile + balances (empty) ───────────────────────────────────

hdr "Account — profile & balances"
PROFILE=$(api_auth GET /account "$TOKEN")
check_field "profile status" "$PROFILE" '.status' 'active'

BALANCES=$(api_auth GET /account/balances "$TOKEN")
check_present "balances data array" "$BALANCES" '.data'

# ── 4. Faucet ─────────────────────────────────────────────────────────────────

hdr "Wallet — faucet deposits"
FAUCET_USDT=$(api_auth POST /wallet/faucet "$TOKEN" -d '{"asset":"USDT","amount":"10000"}')
check_present "faucet USDT depositId" "$FAUCET_USDT" '.depositId'
check_field   "faucet USDT asset"     "$FAUCET_USDT" '.asset' 'USDT'

FAUCET_BTC=$(api_auth POST /wallet/faucet "$TOKEN" -d '{"asset":"BTC","amount":"1"}')
check_present "faucet BTC depositId"  "$FAUCET_BTC" '.depositId'

# Verify balance updated
BALANCES2=$(api_auth GET /account/balances "$TOKEN")
USDT_BAL=$(echo "$BALANCES2" | jq -r '.data[] | select(.asset=="USDT") | .available')
if [ "$(echo "$USDT_BAL > 0" | bc -l 2>/dev/null || echo 0)" = "1" ]; then
  grn "USDT balance > 0 after faucet ($USDT_BAL)"
else
  red "USDT balance not updated (got: $USDT_BAL)"
fi

# ── 5. Markets ────────────────────────────────────────────────────────────────

hdr "Markets — list + detail + orderbook + trades"
MARKETS=$(api GET /markets)
MARKET_COUNT=$(echo "$MARKETS" | jq '.data | length')
if [ "$MARKET_COUNT" -gt 0 ] 2>/dev/null; then
  grn "markets list ($MARKET_COUNT markets)"
else
  red "markets list empty or failed"
fi

BTCUSDT=$(api GET /markets/BTCUSDT)
check_field "BTCUSDT symbol"  "$BTCUSDT" '.symbol'  'BTCUSDT'
check_field "BTCUSDT status"  "$BTCUSDT" '.status'  'trading'

ORDERBOOK=$(api GET /markets/BTCUSDT/orderbook?depth=10)
check_present "orderbook seq"  "$ORDERBOOK" '.seq'
check_present "orderbook bids" "$ORDERBOOK" '.bids'
check_present "orderbook asks" "$ORDERBOOK" '.asks'

TRADES=$(api GET /markets/BTCUSDT/trades?limit=5)
check_present "trades data" "$TRADES" '.data'

KLINES=$(api GET "/markets/BTCUSDT/klines?interval=1m&limit=5")
check_present "klines data" "$KLINES" '.data'

# ── 6. Place order ────────────────────────────────────────────────────────────

hdr "Orders — place limit BUY"
# Place a limit BUY at a price well below market so it rests.
ORDER_BODY='{"market":"BTCUSDT","side":"buy","type":"limit","price":"1000","quantity":"0.001","clientOrderId":"smoke-buy-1"}'
PLACE=$(api_auth POST /orders "$TOKEN" -d "$ORDER_BODY")
check_present "place order id"     "$PLACE" '.id'
check_field   "place order status" "$PLACE" '.status' 'new'
check_field   "place order side"   "$PLACE" '.side'   'buy'
ORDER_ID=$(echo "$PLACE" | jq -r '.id')

# Idempotency — same clientOrderId returns same order
PLACE2=$(api_auth POST /orders "$TOKEN" -d "$ORDER_BODY")
ORDER_ID2=$(echo "$PLACE2" | jq -r '.id')
if [ "$ORDER_ID" = "$ORDER_ID2" ]; then
  grn "idempotency: duplicate clientOrderId returns same order ($ORDER_ID)"
else
  red "idempotency failed: first=$ORDER_ID second=$ORDER_ID2"
fi

# ── 7. Get + list orders ──────────────────────────────────────────────────────

hdr "Orders — get + list"
GET_ORDER=$(api_auth GET "/orders/$ORDER_ID" "$TOKEN")
check_field "get order id"     "$GET_ORDER" '.id'     "$ORDER_ID"
check_field "get order status" "$GET_ORDER" '.status' 'new'

LIST_ORDERS=$(api_auth GET /orders?status=new "$TOKEN")
OPEN_COUNT=$(echo "$LIST_ORDERS" | jq '.data | length')
if [ "$OPEN_COUNT" -ge 1 ] 2>/dev/null; then
  grn "list orders open ($OPEN_COUNT open)"
else
  red "list orders empty (expected ≥1)"
fi

# ── 8. Cancel order ───────────────────────────────────────────────────────────

hdr "Orders — cancel"
CANCEL=$(api_auth DELETE "/orders/$ORDER_ID" "$TOKEN")
check_field "cancel status" "$CANCEL" '.status' 'canceled'

# Idempotent cancel — cancelling an already-canceled order is fine
CANCEL2=$(api_auth DELETE "/orders/$ORDER_ID" "$TOKEN")
check_field "cancel idempotent" "$CANCEL2" '.status' 'canceled'

# Verify it's gone from open list
LIST2=$(api_auth GET /orders?status=new "$TOKEN")
STILL_OPEN=$(echo "$LIST2" | jq "[.data[] | select(.id==\"$ORDER_ID\")] | length")
if [ "$STILL_OPEN" = "0" ]; then
  grn "canceled order no longer in open list"
else
  red "canceled order still in open list"
fi

# ── 9. Account history ────────────────────────────────────────────────────────

hdr "Account — ledger history"
HISTORY=$(api_auth GET /account/history "$TOKEN")
ENTRY_COUNT=$(echo "$HISTORY" | jq '.data | length')
if [ "$ENTRY_COUNT" -ge 2 ] 2>/dev/null; then
  grn "ledger history ($ENTRY_COUNT entries from deposits)"
else
  red "ledger history too short (got $ENTRY_COUNT, expected ≥2)"
fi

# ── 10. WebSocket — snapshot + subscribe ─────────────────────────────────────

hdr "WebSocket — book snapshot on subscribe"
WS_BIN=$(command -v websocat 2>/dev/null || command -v ~/.cargo/bin/websocat 2>/dev/null || echo "")
if [ -n "$WS_BIN" ]; then
  # Send subscribe, keep stdin open for 3 s so the server can respond.
  WS_OUT=$({ echo '{"id":"1","method":"subscribe","params":{"channels":["book.BTCUSDT.diff"]}}'; sleep 3; } \
    | "$WS_BIN" "ws://127.0.0.1:8080/ws" 2>/dev/null || true)
  if echo "$WS_OUT" | grep -q '"snapshot"'; then
    grn "WS received book snapshot"
  elif echo "$WS_OUT" | grep -q '"channels"'; then
    grn "WS subscribe ack received"
  else
    red "WS no useful frame received: $(echo "$WS_OUT" | head -c 200)"
  fi
else
  printf '\033[0;33m~ WS test skipped (websocat not installed — run: cargo install websocat)\033[0m\n'
fi

# ── Summary ───────────────────────────────────────────────────────────────────

printf '\n\033[1m── Day 6 smoke: %d passed, %d failed ──\033[0m\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
