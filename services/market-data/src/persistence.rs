use sqlx::postgres::PgPool;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::binance::KlineUpdate;

const NANOS_PER_MILLISECOND: i128 = 1_000_000;

/// Active market metadata needed by market-data and market-maker workers.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MarketRow {
    /// Market symbol.
    pub symbol: String,
}

/// Market-data persistence boundary.
#[derive(Debug, Clone)]
pub struct MarketDataRepository {
    pool: PgPool,
}

/// Persistence error.
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Binance millisecond timestamp cannot be represented safely.
    #[error("invalid millisecond timestamp")]
    InvalidTimestamp,
}

impl MarketDataRepository {
    /// Creates a repository backed by `pool`.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Loads active configured exchange markets.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when Postgres cannot be queried.
    pub async fn load_active_markets(&self) -> Result<Vec<MarketRow>, PersistenceError> {
        sqlx::query_as::<_, MarketRow>(
            r"
            SELECT symbol
            FROM markets
            WHERE status = 'trading'
            ORDER BY symbol
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(PersistenceError::from)
    }

    /// Finds the demo market-maker user.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when the user row is absent or Postgres fails.
    pub async fn market_maker_user_id(&self) -> Result<Uuid, PersistenceError> {
        sqlx::query_scalar::<_, Uuid>(
            r"
            SELECT id
            FROM users
            WHERE email = 'mm@aether.local' AND is_mm = true
            ",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(PersistenceError::from)
    }

    /// Persists a Binance kline update idempotently.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when timestamps are invalid or Postgres fails.
    pub async fn upsert_kline(&self, kline: &KlineUpdate) -> Result<(), PersistenceError> {
        let opened_at = millis_to_offset(kline.open_time_ms)?;
        let closed_at = millis_to_offset(kline.close_time_ms)?;
        sqlx::query(
            r"
            INSERT INTO klines
              (symbol, interval, opened_at, closed_at, open, high, low, close, volume, is_closed)
            VALUES ($1, $2::kline_interval, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (symbol, interval, opened_at)
            DO UPDATE SET
              closed_at = EXCLUDED.closed_at,
              open = EXCLUDED.open,
              high = EXCLUDED.high,
              low = EXCLUDED.low,
              close = EXCLUDED.close,
              volume = EXCLUDED.volume,
              is_closed = EXCLUDED.is_closed,
              updated_at = now()
            ",
        )
        .bind(&kline.symbol)
        .bind(&kline.interval)
        .bind(opened_at)
        .bind(closed_at)
        .bind(kline.open)
        .bind(kline.high)
        .bind(kline.low)
        .bind(kline.close)
        .bind(kline.volume)
        .bind(kline.is_closed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn millis_to_offset(ms: i64) -> Result<OffsetDateTime, PersistenceError> {
    let nanos = i128::from(ms)
        .checked_mul(NANOS_PER_MILLISECOND)
        .ok_or(PersistenceError::InvalidTimestamp)?;
    OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| PersistenceError::InvalidTimestamp)
}
