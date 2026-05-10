use sqlx::postgres::PgPool;
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    AccountStatus, AuthRepository, AuthUser, CreateRefreshToken, CreateUser, RefreshToken,
    RepositoryError,
};

/// SQLx-backed auth repository.
#[derive(Debug, Clone)]
pub struct PgAuthRepository {
    pool: PgPool,
}

#[derive(Debug, sqlx::FromRow)]
struct AuthUserRow {
    id: Uuid,
    email: String,
    password_hash: String,
    status: String,
    kyc_level: i32,
    totp_enabled: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct RefreshTokenRow {
    id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
    token_hash: String,
    family_id: Uuid,
    expires_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
}

impl PgAuthRepository {
    /// Creates a repository backed by `pool`.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AuthRepository for PgAuthRepository {
    async fn create_user(&self, input: CreateUser) -> Result<AuthUser, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|error| map_sqlx(&error))?;
        let row = sqlx::query_as::<_, AuthUserRow>(
            r"
            INSERT INTO users (id, email, password_hash)
            VALUES ($1, $2, $3)
            RETURNING id, email, password_hash, status::TEXT AS status, kyc_level, totp_enabled
            ",
        )
        .bind(input.id)
        .bind(input.email)
        .bind(input.password_hash)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| map_sqlx(&error))?;

        seed_zero_balances(&mut tx, row.id).await?;
        tx.commit().await.map_err(|error| map_sqlx(&error))?;
        Ok(row.into())
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<AuthUser>, RepositoryError> {
        sqlx::query_as::<_, AuthUserRow>(
            r"
            SELECT id, email, password_hash, status::TEXT AS status, kyc_level, totp_enabled
            FROM users
            WHERE email = $1
            ",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| map_sqlx(&error))
    }

    async fn insert_refresh_token(&self, input: CreateRefreshToken) -> Result<(), RepositoryError> {
        sqlx::query(
            r"
            INSERT INTO refresh_tokens
              (id, user_id, session_id, token_hash, family_id, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ",
        )
        .bind(input.id)
        .bind(input.user_id)
        .bind(input.session_id)
        .bind(input.token_hash)
        .bind(input.family_id)
        .bind(input.expires_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|error| map_sqlx(&error))
    }

    async fn find_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshToken>, RepositoryError> {
        sqlx::query_as::<_, RefreshTokenRow>(
            r"
            SELECT id, user_id, session_id, token_hash, family_id, expires_at, revoked_at
            FROM refresh_tokens
            WHERE token_hash = $1
            ",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| map_sqlx(&error))
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<AuthUser>, RepositoryError> {
        sqlx::query_as::<_, AuthUserRow>(
            r"
            SELECT id, email, password_hash, status::TEXT AS status, kyc_level, totp_enabled
            FROM users
            WHERE id = $1
            ",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| map_sqlx(&error))
    }

    async fn revoke_refresh_token(
        &self,
        token_id: Uuid,
        revoked_at: OffsetDateTime,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r"
            UPDATE refresh_tokens
            SET revoked_at = COALESCE(revoked_at, $2)
            WHERE id = $1
            ",
        )
        .bind(token_id)
        .bind(revoked_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|error| map_sqlx(&error))
    }

    async fn revoke_refresh_family(
        &self,
        family_id: Uuid,
        revoked_at: OffsetDateTime,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r"
            UPDATE refresh_tokens
            SET revoked_at = COALESCE(revoked_at, $2)
            WHERE family_id = $1
            ",
        )
        .bind(family_id)
        .bind(revoked_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|error| map_sqlx(&error))
    }
}

async fn seed_zero_balances(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), RepositoryError> {
    sqlx::query(
        r"
        INSERT INTO balances (user_id, asset_id, available, locked)
        SELECT $1, id, 0, 0
        FROM assets
        WHERE status = 'active'
        ON CONFLICT (user_id, asset_id) DO NOTHING
        ",
    )
    .bind(user_id)
    .execute(tx.as_mut())
    .await
    .map(|_| ())
    .map_err(|error| map_sqlx(&error))
}

fn map_sqlx(error: &sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(db_error) = error {
        if db_error.code().as_deref() == Some("23505") {
            return RepositoryError::EmailTaken;
        }
    }
    tracing::error!(error = %error, "auth.repository.error");
    RepositoryError::Unavailable
}

impl From<AuthUserRow> for AuthUser {
    fn from(row: AuthUserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            password_hash: row.password_hash,
            status: AccountStatus::from_db(&row.status),
            kyc_level: row.kyc_level,
            totp_enabled: row.totp_enabled,
        }
    }
}

impl From<RefreshTokenRow> for RefreshToken {
    fn from(row: RefreshTokenRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            session_id: row.session_id,
            token_hash: row.token_hash,
            family_id: row.family_id,
            expires_at: row.expires_at,
            revoked_at: row.revoked_at,
        }
    }
}
