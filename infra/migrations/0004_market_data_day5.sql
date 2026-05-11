CREATE TYPE kline_interval AS ENUM ('1m', '5m', '15m', '1h', '4h', '1d');

CREATE TABLE klines (
  symbol TEXT NOT NULL,
  interval kline_interval NOT NULL,
  opened_at TIMESTAMPTZ NOT NULL,
  closed_at TIMESTAMPTZ NOT NULL,
  open NUMERIC(38, 18) NOT NULL CHECK (open > 0),
  high NUMERIC(38, 18) NOT NULL CHECK (high > 0),
  low NUMERIC(38, 18) NOT NULL CHECK (low > 0),
  close NUMERIC(38, 18) NOT NULL CHECK (close > 0),
  volume NUMERIC(38, 18) NOT NULL CHECK (volume >= 0),
  is_closed BOOLEAN NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (symbol, interval, opened_at),
  CONSTRAINT chk_klines_high_low CHECK (high >= low)
);

CREATE INDEX idx_klines_symbol_interval_opened
ON klines(symbol, interval, opened_at DESC);

INSERT INTO users (email, password_hash, is_mm, email_verified)
VALUES
  ('mm@aether.local', 'disabled-market-maker-login', true, true),
  ('fees@aether.local', 'disabled-fees-login', false, true)
ON CONFLICT (email) DO NOTHING;

INSERT INTO balances (user_id, asset_id, available, locked)
SELECT users.id, assets.id, 1000000000000, 0
FROM users
CROSS JOIN assets
WHERE users.email = 'mm@aether.local'
  AND assets.status = 'active'
ON CONFLICT (user_id, asset_id)
DO UPDATE SET available = GREATEST(balances.available, EXCLUDED.available);

INSERT INTO ledger_entries
  (user_id, asset_id, kind, amount, available_after, locked_after, reference_type)
SELECT users.id, assets.id, 'adjustment', 1000000000000, balances.available, balances.locked, 'market_maker_bootstrap'
FROM users
JOIN balances ON balances.user_id = users.id
JOIN assets ON assets.id = balances.asset_id
WHERE users.email = 'mm@aether.local'
  AND assets.status = 'active'
  AND NOT EXISTS (
    SELECT 1
    FROM ledger_entries existing
    WHERE existing.user_id = users.id
      AND existing.asset_id = assets.id
      AND existing.reference_type = 'market_maker_bootstrap'
  );
