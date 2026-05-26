use sqlx::PgPool;
use uuid::Uuid;

/// Minimal admin authorization row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AdminUserRow {
    pub id: Uuid,
    pub is_admin: bool,
    pub status: String,
}

/// Market row used by admin controls.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AdminMarketRow {
    pub id: Uuid,
    pub symbol: String,
    pub status: String,
}

/// Engine-state summary row for a market.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MarketStateRow {
    pub symbol: String,
    pub status: String,
    pub open_order_count: i64,
}

/// Finds a user for admin authorization.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn find_admin_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<AdminUserRow>, sqlx::Error> {
    sqlx::query_as::<_, AdminUserRow>(
        r"
        SELECT id, is_admin, status::TEXT
        FROM users
        WHERE id = $1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Finds a market by symbol.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn find_market(
    pool: &PgPool,
    symbol: &str,
) -> Result<Option<AdminMarketRow>, sqlx::Error> {
    sqlx::query_as::<_, AdminMarketRow>(
        r"
        SELECT id, symbol, status::TEXT
        FROM markets
        WHERE symbol = $1
        ",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await
}

/// Updates a market status.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn update_market_status(
    pool: &PgPool,
    symbol: &str,
    status: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r"
        UPDATE markets
        SET status = $2::market_status
        WHERE symbol = $1
        ",
    )
    .bind(symbol)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Freezes a user account.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn freeze_user(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r"
        UPDATE users
        SET status = 'frozen', updated_at = now()
        WHERE id = $1 AND status <> 'closed'
        ",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Returns user IDs with open orders in a market.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn users_with_open_orders_in_market(
    pool: &PgPool,
    symbol: &str,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        r"
        SELECT DISTINCT o.user_id
        FROM orders o
        JOIN markets m ON m.id = o.market_id
        WHERE m.symbol = $1
          AND o.status IN ('pending', 'new', 'partial')
        ORDER BY o.user_id
        ",
    )
    .bind(symbol)
    .fetch_all(pool)
    .await
}

/// Marks returned engine-canceled orders as canceled in the read model.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn mark_orders_canceled(pool: &PgPool, order_ids: &[Uuid]) -> Result<(), sqlx::Error> {
    if order_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r"
        UPDATE orders
        SET status = 'canceled', updated_at = now()
        WHERE id = ANY($1)
          AND status IN ('pending', 'new', 'partial')
        ",
    )
    .bind(order_ids)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Writes an admin audit-log entry.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn insert_audit_log(
    pool: &PgPool,
    actor_user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO audit_log (actor_user_id, action, target_type, target_id, metadata)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Returns per-market open-order counts for the admin engine-state view.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn market_state(pool: &PgPool) -> Result<Vec<MarketStateRow>, sqlx::Error> {
    sqlx::query_as::<_, MarketStateRow>(
        r"
        SELECT
            m.symbol,
            m.status::TEXT AS status,
            COUNT(o.id)::BIGINT AS open_order_count
        FROM markets m
        LEFT JOIN orders o
               ON o.market_id = m.id
              AND o.status IN ('pending', 'new', 'partial')
        WHERE m.status <> 'delisted'
        GROUP BY m.symbol, m.status
        ORDER BY m.symbol
        ",
    )
    .fetch_all(pool)
    .await
}
