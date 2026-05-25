use rust_decimal::Decimal;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

/// A single order row as returned to handlers.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub market_id: Uuid,
    pub symbol: String,
    pub client_order_id: Option<String>,
    pub side: String,
    pub order_type: String,
    pub status: String,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub quantity: Option<Decimal>,
    pub quote_quantity: Option<Decimal>,
    pub display_quantity: Option<Decimal>,
    pub filled_quantity: Decimal,
    pub avg_fill_price: Option<Decimal>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Input for inserting a new pending order.
#[derive(Debug)]
pub struct InsertOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub market_id: Uuid,
    pub client_order_id: Option<String>,
    pub side: String,
    pub order_type: String,
    pub status: String,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub quantity: Option<Decimal>,
    pub quote_quantity: Option<Decimal>,
    pub display_quantity: Option<Decimal>,
}

/// Inserts a new order row and returns it with the market symbol.
///
/// Uses a CTE so we can JOIN markets in the SELECT — `PostgreSQL` RETURNING
/// cannot reference other tables directly.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn insert(pool: &PgPool, input: InsertOrder) -> Result<OrderRow, sqlx::Error> {
    sqlx::query_as::<_, OrderRow>(
        r#"
        WITH ins AS (
            INSERT INTO orders (
                id, user_id, market_id, client_order_id,
                side, "type", status,
                price, stop_price, quantity, quote_quantity, display_quantity,
                filled_quantity
            )
            VALUES (
                $1, $2, $3, $4,
                $5::order_side, $6::order_type, $7::order_status,
                $8, $9, $10, $11, $12,
                0
            )
            RETURNING *
        )
        SELECT
            ins.id,
            ins.user_id,
            ins.market_id,
            m.symbol,
            ins.client_order_id,
            ins.side::TEXT            AS side,
            ins."type"::TEXT          AS order_type,
            ins.status::TEXT          AS status,
            ins.price,
            ins.stop_price,
            ins.quantity,
            ins.quote_quantity,
            ins.display_quantity,
            ins.filled_quantity,
            ins.avg_fill_price,
            ins.created_at,
            ins.updated_at
        FROM ins
        JOIN markets m ON m.id = ins.market_id
        "#,
    )
    .bind(input.id)
    .bind(input.user_id)
    .bind(input.market_id)
    .bind(&input.client_order_id)
    .bind(&input.side)
    .bind(&input.order_type)
    .bind(&input.status)
    .bind(input.price)
    .bind(input.stop_price)
    .bind(input.quantity)
    .bind(input.quote_quantity)
    .bind(input.display_quantity)
    .fetch_one(pool)
    .await
}

/// Finds an order by its public ID, scoped to `user_id`.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn find_by_id(
    pool: &PgPool,
    order_id: Uuid,
    user_id: Uuid,
) -> Result<Option<OrderRow>, sqlx::Error> {
    sqlx::query_as::<_, OrderRow>(
        r#"
        SELECT
            o.id,
            o.user_id,
            o.market_id,
            m.symbol,
            o.client_order_id,
            o.side::TEXT           AS side,
            o."type"::TEXT         AS order_type,
            o.status::TEXT         AS status,
            o.price,
            o.stop_price,
            o.quantity,
            o.quote_quantity,
            o.display_quantity,
            o.filled_quantity,
            o.avg_fill_price,
            o.created_at,
            o.updated_at
        FROM orders o
        JOIN markets m ON m.id = o.market_id
        WHERE o.id = $1 AND o.user_id = $2
        "#,
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Finds an order by `(user_id, client_order_id)` for idempotency checks.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn find_by_client_order_id(
    pool: &PgPool,
    user_id: Uuid,
    client_order_id: &str,
) -> Result<Option<OrderRow>, sqlx::Error> {
    sqlx::query_as::<_, OrderRow>(
        r#"
        SELECT
            o.id,
            o.user_id,
            o.market_id,
            m.symbol,
            o.client_order_id,
            o.side::TEXT           AS side,
            o."type"::TEXT         AS order_type,
            o.status::TEXT         AS status,
            o.price,
            o.stop_price,
            o.quantity,
            o.quote_quantity,
            o.display_quantity,
            o.filled_quantity,
            o.avg_fill_price,
            o.created_at,
            o.updated_at
        FROM orders o
        JOIN markets m ON m.id = o.market_id
        WHERE o.user_id = $1 AND o.client_order_id = $2
        "#,
    )
    .bind(user_id)
    .bind(client_order_id)
    .fetch_optional(pool)
    .await
}

/// Lists orders for a user with optional filters and cursor pagination.
///
/// Returns at most `limit` rows ordered by `created_at DESC`.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    status_filter: Option<&str>,
    market_filter: Option<&str>,
    cursor: Option<OffsetDateTime>,
    limit: i64,
) -> Result<Vec<OrderRow>, sqlx::Error> {
    sqlx::query_as::<_, OrderRow>(
        r#"
        SELECT
            o.id,
            o.user_id,
            o.market_id,
            m.symbol,
            o.client_order_id,
            o.side::TEXT           AS side,
            o."type"::TEXT         AS order_type,
            o.status::TEXT         AS status,
            o.price,
            o.stop_price,
            o.quantity,
            o.quote_quantity,
            o.display_quantity,
            o.filled_quantity,
            o.avg_fill_price,
            o.created_at,
            o.updated_at
        FROM orders o
        JOIN markets m ON m.id = o.market_id
        WHERE o.user_id = $1
          AND ($2::TEXT IS NULL OR o.status::TEXT = $2)
          AND ($3::TEXT IS NULL OR m.symbol = $3)
          AND ($4::TIMESTAMPTZ IS NULL OR o.created_at < $4)
        ORDER BY o.created_at DESC
        LIMIT $5
        "#,
    )
    .bind(user_id)
    .bind(status_filter)
    .bind(market_filter)
    .bind(cursor)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Updates an order status.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn update_status(pool: &PgPool, order_id: Uuid, status: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE orders SET status = $1::order_status, updated_at = now() WHERE id = $2")
        .bind(status)
        .bind(order_id)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Atomically checks and locks `amount` of `asset_id` for `user_id`.
///
/// Deducts from `available`, adds to `locked`. Returns `false` when the
/// balance is insufficient (no row updated).
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn try_lock_balance(
    pool: &PgPool,
    user_id: Uuid,
    asset_id: Uuid,
    amount: Decimal,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, bool>(
        r"
        UPDATE balances
        SET
            available  = available - $3,
            locked     = locked    + $3,
            updated_at = now()
        WHERE user_id  = $1
          AND asset_id = $2
          AND available >= $3
        RETURNING true
        ",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .fetch_optional(pool)
    .await?;
    Ok(row.unwrap_or(false))
}

/// Releases `amount` of locked balance back to available.
///
/// Called on cancel or engine rejection after a successful lock.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn unlock_balance(
    pool: &PgPool,
    user_id: Uuid,
    asset_id: Uuid,
    amount: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        UPDATE balances
        SET
            available  = available + $3,
            locked     = locked    - $3,
            updated_at = now()
        WHERE user_id  = $1
          AND asset_id = $2
        ",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .execute(pool)
    .await
    .map(|_| ())
}
