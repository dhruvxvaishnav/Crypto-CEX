use rust_decimal::Decimal;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

// ── API key types ─────────────────────────────────────────────────────────────

/// Minimal API key row returned by lookup queries.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub permissions: Vec<String>,
    pub created_at: OffsetDateTime,
}

/// Fetches an active API key by ID.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn find_api_key(pool: &PgPool, key_id: Uuid) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        r"
        SELECT id, user_id, label, permissions, created_at
        FROM api_keys
        WHERE id = $1 AND revoked_at IS NULL
        ",
    )
    .bind(key_id)
    .fetch_optional(pool)
    .await
}

/// Decrypts the API key secret using pgcrypto `pgp_sym_decrypt`.
///
/// Returns `None` if the key has no secret (should never happen in practice).
///
/// # Errors
///
/// Returns `sqlx::Error` on DB or decryption failure.
pub async fn decrypt_api_key_secret(
    pool: &PgPool,
    key_id: Uuid,
    pgcrypto_key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, Option<String>>(
        r"
        SELECT pgp_sym_decrypt(secret_hash::bytea, $2)
        FROM api_keys
        WHERE id = $1 AND revoked_at IS NULL
        ",
    )
    .bind(key_id)
    .bind(pgcrypto_key)
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
}

/// Inserts a new API key with the secret stored via pgcrypto.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn create_api_key(
    pool: &PgPool,
    user_id: Uuid,
    label: &str,
    raw_secret: &str,
    permissions: &[String],
    pgcrypto_key: &str,
) -> Result<ApiKeyRow, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        r"
        INSERT INTO api_keys (user_id, label, secret_hash, permissions)
        VALUES ($1, $2, pgp_sym_encrypt($3, $4), $5)
        RETURNING id, user_id, label, permissions, created_at
        ",
    )
    .bind(user_id)
    .bind(label)
    .bind(raw_secret)
    .bind(pgcrypto_key)
    .bind(permissions)
    .fetch_one(pool)
    .await
}

/// Revokes an API key (sets `revoked_at`).
///
/// Returns true if a key was revoked, false if not found.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn revoke_api_key(
    pool: &PgPool,
    user_id: Uuid,
    key_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r"
        UPDATE api_keys
        SET revoked_at = now()
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        ",
    )
    .bind(key_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Lists active API keys for a user (does not return the secret).
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn list_api_keys(pool: &PgPool, user_id: Uuid) -> Result<Vec<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        r"
        SELECT id, user_id, label, permissions, created_at
        FROM api_keys
        WHERE user_id = $1 AND revoked_at IS NULL
        ORDER BY created_at DESC
        ",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

// ── TOTP types ────────────────────────────────────────────────────────────────

/// Decrypts the TOTP secret for a user using pgcrypto `pgp_sym_decrypt`.
///
/// Returns `None` if the user has no TOTP secret set.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB or decryption failure.
pub async fn decrypt_totp_secret(
    pool: &PgPool,
    user_id: Uuid,
    pgcrypto_key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, Option<String>>(
        r"
        SELECT pgp_sym_decrypt(totp_secret_encrypted, $2)::text
        FROM users
        WHERE id = $1 AND totp_secret_encrypted IS NOT NULL
        ",
    )
    .bind(user_id)
    .bind(pgcrypto_key)
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
}

/// Stores the TOTP secret (encrypted) and marks setup as pending.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn store_totp_secret(
    pool: &PgPool,
    user_id: Uuid,
    secret_base32: &str,
    pgcrypto_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        UPDATE users
        SET totp_secret_encrypted = pgp_sym_encrypt($2, $3)::bytea,
            updated_at = now()
        WHERE id = $1
        ",
    )
    .bind(user_id)
    .bind(secret_base32)
    .bind(pgcrypto_key)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Enables TOTP for a user (sets `totp_enabled = true`).
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn enable_totp(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        UPDATE users SET totp_enabled = true, updated_at = now() WHERE id = $1
        ",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Returns a user row with `password_hash` for re-authentication flows.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn get_user_for_auth(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<UserForAuth>, sqlx::Error> {
    sqlx::query_as::<_, UserForAuth>(
        r"
        SELECT id, email, password_hash, totp_enabled
        FROM users
        WHERE id = $1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// User row for re-authentication (password check).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserForAuth {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub totp_enabled: bool,
}

/// Disables TOTP for a user (clears secret and flag).
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn disable_totp(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        UPDATE users
        SET totp_enabled = false,
            totp_secret_encrypted = NULL,
            updated_at = now()
        WHERE id = $1
        ",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// User profile row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserProfileRow {
    pub id: Uuid,
    pub email: String,
    pub status: String,
    pub kyc_level: i32,
    pub email_verified: bool,
    pub totp_enabled: bool,
    pub is_admin: bool,
    pub created_at: OffsetDateTime,
}

/// Balance row for a single asset.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BalanceRow {
    pub asset_id: Uuid,
    pub asset_symbol: String,
    pub asset_name: String,
    pub available: Decimal,
    pub locked: Decimal,
    pub updated_at: OffsetDateTime,
}

/// Unified ledger history entry.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LedgerRow {
    pub id: i64,
    pub asset_symbol: String,
    pub kind: String,
    pub amount: Decimal,
    pub available_after: Decimal,
    pub locked_after: Decimal,
    pub reference_type: String,
    pub reference_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
}

/// Fetches the user profile.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn get_profile(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<UserProfileRow>, sqlx::Error> {
    sqlx::query_as::<_, UserProfileRow>(
        r"
        SELECT id, email, status::TEXT, kyc_level, email_verified, totp_enabled, is_admin, created_at
        FROM users
        WHERE id = $1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Fetches all balances for a user, including zero balances for seeded assets.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn list_balances(pool: &PgPool, user_id: Uuid) -> Result<Vec<BalanceRow>, sqlx::Error> {
    sqlx::query_as::<_, BalanceRow>(
        r"
        SELECT
            b.asset_id,
            a.symbol AS asset_symbol,
            a.name   AS asset_name,
            b.available,
            b.locked,
            b.updated_at
        FROM balances b
        JOIN assets a ON a.id = b.asset_id
        WHERE b.user_id = $1
        ORDER BY a.symbol
        ",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Returns a paginated cursor page of ledger history for a user.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn ledger_history(
    pool: &PgPool,
    user_id: Uuid,
    cursor: Option<i64>,
    limit: i64,
) -> Result<Vec<LedgerRow>, sqlx::Error> {
    sqlx::query_as::<_, LedgerRow>(
        r"
        SELECT
            le.id,
            a.symbol AS asset_symbol,
            le.kind::TEXT,
            le.amount,
            le.available_after,
            le.locked_after,
            le.reference_type,
            le.reference_id,
            le.created_at
        FROM ledger_entries le
        JOIN assets a ON a.id = le.asset_id
        WHERE le.user_id = $1
          AND ($2::BIGINT IS NULL OR le.id < $2)
        ORDER BY le.id DESC
        LIMIT $3
        ",
    )
    .bind(user_id)
    .bind(cursor)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Applies a faucet deposit: credits `amount` of `asset_id` to the user's
/// available balance and writes a ledger entry, all in one transaction.
///
/// Returns the new available balance.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure or constraint violation.
pub async fn apply_faucet(
    pool: &PgPool,
    user_id: Uuid,
    asset_id: Uuid,
    amount: Decimal,
    deposit_id: Uuid,
) -> Result<Decimal, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let new_available = sqlx::query_scalar::<_, Decimal>(
        r"
        UPDATE balances
        SET available = available + $3, updated_at = now()
        WHERE user_id = $1 AND asset_id = $2
        RETURNING available
        ",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .fetch_one(tx.as_mut())
    .await?;

    // Record deposit row.
    sqlx::query(
        r"
        INSERT INTO deposits (id, user_id, asset_id, amount, status, completed_at)
        VALUES ($1, $2, $3, $4, 'completed', now())
        ",
    )
    .bind(deposit_id)
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .execute(tx.as_mut())
    .await?;

    // Ledger entry.
    sqlx::query(
        r"
        INSERT INTO ledger_entries
            (user_id, asset_id, kind, amount, available_after, locked_after,
             reference_type, reference_id)
        SELECT $1, $2, 'deposit', $3, b.available, b.locked, 'deposit', $4
        FROM balances b
        WHERE b.user_id = $1 AND b.asset_id = $2
        ",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .bind(deposit_id)
    .execute(tx.as_mut())
    .await?;

    tx.commit().await?;
    Ok(new_available)
}

/// Returns the faucet limit for an asset.
///
/// # Errors
///
/// Returns `sqlx::Error` on DB failure.
pub async fn get_faucet_limit(
    pool: &PgPool,
    asset_symbol: &str,
) -> Result<Option<(Uuid, Decimal)>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        faucet_max: Decimal,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT id, faucet_max FROM assets WHERE symbol = $1 AND status = 'active'",
    )
    .bind(asset_symbol)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (r.id, r.faucet_max)))
}
