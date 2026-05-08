INSERT INTO assets (symbol, name, decimals, faucet_max, min_withdrawal, withdraw_regex)
VALUES
  ('BTC', 'Bitcoin', 8, 1, 0.0001, '^(bc1|[13])[a-zA-HJ-NP-Z0-9]{25,62}$'),
  ('ETH', 'Ethereum', 18, 20, 0.001, '^0x[a-fA-F0-9]{40}$'),
  ('SOL', 'Solana', 9, 500, 0.01, '^[1-9A-HJ-NP-Za-km-z]{32,44}$'),
  ('USDT', 'Tether USD', 6, 100000, 10, '^0x[a-fA-F0-9]{40}$')
ON CONFLICT (symbol) DO NOTHING;

INSERT INTO markets (symbol, base_asset_id, quote_asset_id, tick_size, lot_size, min_notional)
SELECT 'BTCUSDT', base.id, quote.id, 0.01, 0.00001, 10
FROM assets base, assets quote
WHERE base.symbol = 'BTC' AND quote.symbol = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

INSERT INTO markets (symbol, base_asset_id, quote_asset_id, tick_size, lot_size, min_notional)
SELECT 'ETHUSDT', base.id, quote.id, 0.01, 0.0001, 10
FROM assets base, assets quote
WHERE base.symbol = 'ETH' AND quote.symbol = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

INSERT INTO markets (symbol, base_asset_id, quote_asset_id, tick_size, lot_size, min_notional)
SELECT 'SOLUSDT', base.id, quote.id, 0.001, 0.01, 5
FROM assets base, assets quote
WHERE base.symbol = 'SOL' AND quote.symbol = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

INSERT INTO worker_state (worker_name)
VALUES ('settlement'), ('market-data')
ON CONFLICT (worker_name) DO NOTHING;
