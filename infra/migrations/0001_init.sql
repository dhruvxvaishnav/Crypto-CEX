CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE account_status AS ENUM ('active', 'frozen', 'closed');
CREATE TYPE asset_status AS ENUM ('active', 'disabled');
CREATE TYPE market_status AS ENUM ('trading', 'halted', 'delisted');
CREATE TYPE order_side AS ENUM ('buy', 'sell');
CREATE TYPE order_status AS ENUM ('pending', 'new', 'partial', 'filled', 'canceled', 'rejected');
CREATE TYPE order_type AS ENUM ('limit', 'market', 'ioc', 'fok', 'post_only', 'stop_limit', 'stop_market', 'oco');
CREATE TYPE stp_mode AS ENUM ('decrement', 'cancel_maker', 'cancel_taker');
CREATE TYPE transfer_status AS ENUM ('pending', 'processing', 'completed', 'canceled', 'rejected');
CREATE TYPE ledger_entry_kind AS ENUM ('deposit', 'withdrawal', 'trade', 'fee', 'lock', 'unlock', 'adjustment');

CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  status account_status NOT NULL DEFAULT 'active',
  kyc_level INTEGER NOT NULL DEFAULT 0 CHECK (kyc_level >= 0),
  email_verified BOOLEAN NOT NULL DEFAULT false,
  totp_enabled BOOLEAN NOT NULL DEFAULT false,
  totp_secret_encrypted BYTEA,
  stp_mode stp_mode NOT NULL DEFAULT 'decrement',
  is_admin BOOLEAN NOT NULL DEFAULT false,
  is_mm BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT uq_users_email UNIQUE (email)
);

CREATE TABLE refresh_tokens (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  session_id UUID NOT NULL,
  token_hash TEXT NOT NULL,
  family_id UUID NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT uq_refresh_tokens_token_hash UNIQUE (token_hash)
);

CREATE TABLE assets (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  symbol TEXT NOT NULL,
  name TEXT NOT NULL,
  status asset_status NOT NULL DEFAULT 'active',
  decimals INTEGER NOT NULL CHECK (decimals BETWEEN 0 AND 18),
  faucet_max NUMERIC(38, 18) NOT NULL CHECK (faucet_max >= 0),
  min_withdrawal NUMERIC(38, 18) NOT NULL CHECK (min_withdrawal >= 0),
  withdraw_regex TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT uq_assets_symbol UNIQUE (symbol)
);

CREATE TABLE markets (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  symbol TEXT NOT NULL,
  base_asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  quote_asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  status market_status NOT NULL DEFAULT 'trading',
  tick_size NUMERIC(38, 18) NOT NULL CHECK (tick_size > 0),
  lot_size NUMERIC(38, 18) NOT NULL CHECK (lot_size > 0),
  min_notional NUMERIC(38, 18) NOT NULL CHECK (min_notional > 0),
  maker_fee_bps NUMERIC(38, 18) NOT NULL DEFAULT 10 CHECK (maker_fee_bps >= 0),
  taker_fee_bps NUMERIC(38, 18) NOT NULL DEFAULT 10 CHECK (taker_fee_bps >= 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT uq_markets_symbol UNIQUE (symbol),
  CONSTRAINT chk_markets_distinct_assets CHECK (base_asset_id <> quote_asset_id)
);

CREATE TABLE balances (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  available NUMERIC(38, 18) NOT NULL DEFAULT 0 CHECK (available >= 0),
  locked NUMERIC(38, 18) NOT NULL DEFAULT 0 CHECK (locked >= 0),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, asset_id)
);

CREATE TABLE orders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  market_id UUID NOT NULL REFERENCES markets(id) ON DELETE RESTRICT,
  client_order_id TEXT,
  side order_side NOT NULL,
  type order_type NOT NULL,
  status order_status NOT NULL,
  price NUMERIC(38, 18) CHECK (price IS NULL OR price > 0),
  stop_price NUMERIC(38, 18) CHECK (stop_price IS NULL OR stop_price > 0),
  quantity NUMERIC(38, 18) CHECK (quantity IS NULL OR quantity > 0),
  quote_quantity NUMERIC(38, 18) CHECK (quote_quantity IS NULL OR quote_quantity > 0),
  display_quantity NUMERIC(38, 18) CHECK (display_quantity IS NULL OR display_quantity > 0),
  filled_quantity NUMERIC(38, 18) NOT NULL DEFAULT 0 CHECK (filled_quantity >= 0),
  avg_fill_price NUMERIC(38, 18) CHECK (avg_fill_price IS NULL OR avg_fill_price >= 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT uq_orders_user_client_order UNIQUE (user_id, client_order_id),
  CONSTRAINT chk_orders_quantity_source CHECK (quantity IS NOT NULL OR quote_quantity IS NOT NULL)
);

CREATE TABLE trades (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  market_id UUID NOT NULL REFERENCES markets(id) ON DELETE RESTRICT,
  taker_order_id UUID NOT NULL REFERENCES orders(id) ON DELETE RESTRICT,
  maker_order_id UUID NOT NULL REFERENCES orders(id) ON DELETE RESTRICT,
  price NUMERIC(38, 18) NOT NULL CHECK (price > 0),
  quantity NUMERIC(38, 18) NOT NULL CHECK (quantity > 0),
  taker_side order_side NOT NULL,
  engine_seq BIGINT NOT NULL UNIQUE CHECK (engine_seq > 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ledger_entries (
  id BIGSERIAL PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  kind ledger_entry_kind NOT NULL,
  amount NUMERIC(38, 18) NOT NULL,
  available_after NUMERIC(38, 18) NOT NULL CHECK (available_after >= 0),
  locked_after NUMERIC(38, 18) NOT NULL CHECK (locked_after >= 0),
  reference_type TEXT NOT NULL,
  reference_id UUID,
  engine_seq BIGINT CHECK (engine_seq IS NULL OR engine_seq > 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE deposits (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  amount NUMERIC(38, 18) NOT NULL CHECK (amount > 0),
  status transfer_status NOT NULL DEFAULT 'pending',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ
);

CREATE TABLE withdrawals (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
  amount NUMERIC(38, 18) NOT NULL CHECK (amount > 0),
  address TEXT NOT NULL,
  status transfer_status NOT NULL DEFAULT 'processing',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ
);

CREATE TABLE api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  secret_hash TEXT NOT NULL,
  permissions TEXT[] NOT NULL,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE proof_of_reserves (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  merkle_root TEXT NOT NULL,
  liabilities JSONB NOT NULL,
  generated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE engine_events (
  seq BIGINT PRIMARY KEY CHECK (seq > 0),
  event_type TEXT NOT NULL,
  payload JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  processed_at TIMESTAMPTZ
);

CREATE TABLE audit_log (
  id BIGSERIAL PRIMARY KEY,
  actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  action TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id UUID,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE worker_state (
  worker_name TEXT PRIMARY KEY,
  last_processed_seq BIGINT NOT NULL DEFAULT 0 CHECK (last_processed_seq >= 0),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_balances_asset_id ON balances(asset_id);
CREATE INDEX idx_orders_user_status ON orders(user_id, status);
CREATE INDEX idx_orders_market_status ON orders(market_id, status);
CREATE INDEX idx_orders_created_at ON orders(created_at);
CREATE INDEX idx_trades_market_created_at ON trades(market_id, created_at DESC);
CREATE INDEX idx_ledger_entries_user_created_at ON ledger_entries(user_id, created_at DESC);
CREATE INDEX idx_deposits_user_created_at ON deposits(user_id, created_at DESC);
CREATE INDEX idx_withdrawals_user_created_at ON withdrawals(user_id, created_at DESC);
CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX idx_engine_events_processed ON engine_events(processed_at, seq);
CREATE INDEX idx_audit_log_actor_created_at ON audit_log(actor_user_id, created_at DESC);
