use rust_decimal::Decimal;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

/// Full market row with 24h rolling stats derived from the klines table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MarketRow {
    pub id: Uuid,
    pub symbol: String,
    pub base_asset_id: Uuid,
    pub quote_asset_id: Uuid,
    pub base_asset_symbol: String,
    pub quote_asset_symbol: String,
    pub status: String,
    pub tick_size: Decimal,
    pub lot_size: Decimal,
    pub min_notional: Decimal,
    pub maker_fee_bps: Decimal,
    pub taker_fee_bps: Decimal,
    pub last_price: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub high_24h: Option<Decimal>,
    pub low_24h: Option<Decimal>,
    pub price_change_pct: Option<Decimal>,
}

/// A single recent trade row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TradeRow {
    pub id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
    pub taker_side: String,
    pub created_at: OffsetDateTime,
}

/// Ordering for trade-history queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeOrder {
    /// Newest trades first.
    Desc,
    /// Oldest trades first.
    Asc,
}

/// A single kline (OHLCV candle) row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KlineRow {
    /// Candle open timestamp (UTC).
    pub ts: OffsetDateTime,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

/// Lists all non-delisted markets with 24h rolling stats from the klines table.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn list_markets(pool: &PgPool) -> Result<Vec<MarketRow>, sqlx::Error> {
    // klines uses `symbol TEXT` and `opened_at` — join on symbol, no market_id FK.
    sqlx::query_as::<_, MarketRow>(
        r"
        WITH stats AS (
            SELECT
                k.symbol,
                (array_agg(k.open  ORDER BY k.opened_at ASC))[1]   AS open_24h,
                MAX(k.high)                                          AS high_24h,
                MIN(k.low)                                           AS low_24h,
                (array_agg(k.close ORDER BY k.opened_at DESC))[1]   AS last_price,
                SUM(k.volume)                                        AS volume_24h
            FROM klines k
            WHERE k.interval = '1m'
              AND k.opened_at >= now() - INTERVAL '24 hours'
            GROUP BY k.symbol
        )
        SELECT
            m.id,
            m.symbol,
            m.base_asset_id,
            m.quote_asset_id,
            ba.symbol               AS base_asset_symbol,
            qa.symbol               AS quote_asset_symbol,
            m.status::TEXT          AS status,
            m.tick_size,
            m.lot_size,
            m.min_notional,
            m.maker_fee_bps,
            m.taker_fee_bps,
            s.last_price,
            s.volume_24h,
            s.high_24h,
            s.low_24h,
            CASE
                WHEN s.open_24h IS NOT NULL AND s.open_24h <> 0
                THEN ROUND(((s.last_price - s.open_24h) / s.open_24h) * 100, 4)
                ELSE NULL
            END                     AS price_change_pct
        FROM markets m
        JOIN assets ba ON ba.id = m.base_asset_id
        JOIN assets qa ON qa.id = m.quote_asset_id
        LEFT JOIN stats s ON s.symbol = m.symbol
        WHERE m.status <> 'delisted'
        ORDER BY m.symbol
        ",
    )
    .fetch_all(pool)
    .await
}

/// Fetches a single market by symbol with 24h stats.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn get_market(pool: &PgPool, symbol: &str) -> Result<Option<MarketRow>, sqlx::Error> {
    sqlx::query_as::<_, MarketRow>(
        r"
        WITH stats AS (
            SELECT
                k.symbol,
                (array_agg(k.open  ORDER BY k.opened_at ASC))[1]   AS open_24h,
                MAX(k.high)                                          AS high_24h,
                MIN(k.low)                                           AS low_24h,
                (array_agg(k.close ORDER BY k.opened_at DESC))[1]   AS last_price,
                SUM(k.volume)                                        AS volume_24h
            FROM klines k
            WHERE k.interval = '1m'
              AND k.opened_at >= now() - INTERVAL '24 hours'
              AND k.symbol = $1
            GROUP BY k.symbol
        )
        SELECT
            m.id,
            m.symbol,
            m.base_asset_id,
            m.quote_asset_id,
            ba.symbol               AS base_asset_symbol,
            qa.symbol               AS quote_asset_symbol,
            m.status::TEXT          AS status,
            m.tick_size,
            m.lot_size,
            m.min_notional,
            m.maker_fee_bps,
            m.taker_fee_bps,
            s.last_price,
            s.volume_24h,
            s.high_24h,
            s.low_24h,
            CASE
                WHEN s.open_24h IS NOT NULL AND s.open_24h <> 0
                THEN ROUND(((s.last_price - s.open_24h) / s.open_24h) * 100, 4)
                ELSE NULL
            END                     AS price_change_pct
        FROM markets m
        JOIN assets ba ON ba.id = m.base_asset_id
        JOIN assets qa ON qa.id = m.quote_asset_id
        LEFT JOIN stats s ON s.symbol = m.symbol
        WHERE m.symbol = $1
        ",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await
}

/// Returns the market's id, base asset id, quote asset id, and status.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn get_market_assets(
    pool: &PgPool,
    symbol: &str,
) -> Result<Option<(Uuid, Uuid, Uuid, String)>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        base_asset_id: Uuid,
        quote_asset_id: Uuid,
        status: String,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT id, base_asset_id, quote_asset_id, status::TEXT FROM markets WHERE symbol = $1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (r.id, r.base_asset_id, r.quote_asset_id, r.status)))
}

/// Returns the most recent public trades for a market, newest first.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn recent_trades(
    pool: &PgPool,
    symbol: &str,
    from: Option<OffsetDateTime>,
    to: Option<OffsetDateTime>,
    limit: i64,
    order: TradeOrder,
) -> Result<Vec<TradeRow>, sqlx::Error> {
    let sql = match order {
        TradeOrder::Desc => {
            r"
        SELECT
            t.id,
            t.price,
            t.quantity,
            t.taker_side::TEXT      AS taker_side,
            t.created_at
        FROM trades t
        JOIN markets m ON m.id = t.market_id
        WHERE m.symbol = $1
          AND ($2::TIMESTAMPTZ IS NULL OR t.created_at >= $2)
          AND ($3::TIMESTAMPTZ IS NULL OR t.created_at <= $3)
        ORDER BY t.created_at DESC
        LIMIT $4
        "
        }
        TradeOrder::Asc => {
            r"
        SELECT
            t.id,
            t.price,
            t.quantity,
            t.taker_side::TEXT      AS taker_side,
            t.created_at
        FROM trades t
        JOIN markets m ON m.id = t.market_id
        WHERE m.symbol = $1
          AND ($2::TIMESTAMPTZ IS NULL OR t.created_at >= $2)
          AND ($3::TIMESTAMPTZ IS NULL OR t.created_at <= $3)
        ORDER BY t.created_at ASC
        LIMIT $4
        "
        }
    };

    sqlx::query_as::<_, TradeRow>(sql)
        .bind(symbol)
        .bind(from)
        .bind(to)
        .bind(limit)
        .fetch_all(pool)
        .await
}

/// Returns klines for a market and interval within `[from, to]`.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn klines(
    pool: &PgPool,
    symbol: &str,
    interval: &str,
    from: OffsetDateTime,
    to: OffsetDateTime,
    limit: i64,
) -> Result<Vec<KlineRow>, sqlx::Error> {
    sqlx::query_as::<_, KlineRow>(
        r"
        SELECT
            k.opened_at             AS ts,
            k.open,
            k.high,
            k.low,
            k.close,
            k.volume
        FROM klines k
        WHERE k.symbol   = $1
          AND k.interval = $2::kline_interval
          AND k.opened_at >= $3
          AND k.opened_at <= $4
        ORDER BY k.opened_at ASC
        LIMIT $5
        ",
    )
    .bind(symbol)
    .bind(interval)
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// 24h ticker snapshot for a single symbol — used by the ticker publisher.
#[derive(Debug, sqlx::FromRow)]
pub struct TickerSnapshot {
    pub symbol: String,
    pub last_price: Option<Decimal>,
    pub open_24h: Option<Decimal>,
    pub high_24h: Option<Decimal>,
    pub low_24h: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
}

/// Returns 24h ticker snapshots for all trading markets.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn ticker_snapshots(pool: &PgPool) -> Result<Vec<TickerSnapshot>, sqlx::Error> {
    sqlx::query_as::<_, TickerSnapshot>(
        r"
        SELECT
            m.symbol,
            (array_agg(k.close ORDER BY k.opened_at DESC))[1] AS last_price,
            (array_agg(k.open  ORDER BY k.opened_at ASC))[1]  AS open_24h,
            MAX(k.high)                                         AS high_24h,
            MIN(k.low)                                          AS low_24h,
            SUM(k.volume)                                       AS volume_24h
        FROM markets m
        LEFT JOIN klines k
               ON k.symbol    = m.symbol
              AND k.interval  = '1m'
              AND k.opened_at >= now() - INTERVAL '24 hours'
        WHERE m.status = 'trading'
        GROUP BY m.symbol
        ORDER BY m.symbol
        ",
    )
    .fetch_all(pool)
    .await
}
