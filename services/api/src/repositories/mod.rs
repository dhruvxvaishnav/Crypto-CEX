use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

pub mod account;
pub mod admin;
pub mod markets;
pub mod orders;
pub mod postgres;
pub mod proof;

#[cfg(test)]
pub mod memory;

/// Shared auth repository.
pub type SharedAuthRepository = std::sync::Arc<dyn AuthRepository>;

/// User row needed by auth.
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// User ID.
    pub id: Uuid,
    /// Normalised email.
    pub email: String,
    /// Argon2id password hash.
    pub password_hash: String,
    /// Account status.
    pub status: AccountStatus,
    /// KYC level.
    pub kyc_level: i32,
    /// Whether TOTP is enabled.
    pub totp_enabled: bool,
}

/// Account status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    /// User can authenticate and trade.
    Active,
    /// User is frozen.
    Frozen,
    /// User is closed.
    Closed,
}

/// Refresh-token row needed by auth.
#[derive(Debug, Clone)]
pub struct RefreshToken {
    /// Token row ID.
    pub id: Uuid,
    /// User ID.
    pub user_id: Uuid,
    /// Session ID.
    pub session_id: Uuid,
    /// SHA-256 token hash.
    pub token_hash: String,
    /// Session-family ID.
    pub family_id: Uuid,
    /// Expiry timestamp.
    pub expires_at: OffsetDateTime,
    /// Revocation timestamp.
    pub revoked_at: Option<OffsetDateTime>,
}

/// New user insert payload.
#[derive(Debug, Clone)]
pub struct CreateUser {
    /// User ID.
    pub id: Uuid,
    /// Normalised email.
    pub email: String,
    /// Argon2id password hash.
    pub password_hash: String,
}

/// New refresh token insert payload.
#[derive(Debug, Clone)]
pub struct CreateRefreshToken {
    /// Token row ID.
    pub id: Uuid,
    /// User ID.
    pub user_id: Uuid,
    /// Session ID.
    pub session_id: Uuid,
    /// SHA-256 token hash.
    pub token_hash: String,
    /// Session-family ID.
    pub family_id: Uuid,
    /// Expiry timestamp.
    pub expires_at: OffsetDateTime,
}

/// Auth repository errors.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Email uniqueness conflict.
    #[error("email already exists")]
    EmailTaken,
    /// Storage operation failed.
    #[error("auth repository unavailable")]
    Unavailable,
}

/// Persistence boundary for auth.
#[async_trait]
pub trait AuthRepository: Send + Sync {
    /// Creates a user and zero balances for active assets atomically.
    async fn create_user(&self, input: CreateUser) -> Result<AuthUser, RepositoryError>;
    /// Finds a user by normalised email.
    async fn find_user_by_email(&self, email: &str) -> Result<Option<AuthUser>, RepositoryError>;
    /// Inserts a refresh token.
    async fn insert_refresh_token(&self, input: CreateRefreshToken) -> Result<(), RepositoryError>;
    /// Finds a refresh token by SHA-256 hash.
    async fn find_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshToken>, RepositoryError>;
    /// Finds a user by ID.
    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<AuthUser>, RepositoryError>;
    /// Revokes one refresh token.
    async fn revoke_refresh_token(
        &self,
        token_id: Uuid,
        revoked_at: OffsetDateTime,
    ) -> Result<(), RepositoryError>;
    /// Revokes all tokens in a session family.
    async fn revoke_refresh_family(
        &self,
        family_id: Uuid,
        revoked_at: OffsetDateTime,
    ) -> Result<(), RepositoryError>;
}

impl AccountStatus {
    fn from_db(value: &str) -> Self {
        match value {
            "active" => Self::Active,
            "frozen" => Self::Frozen,
            _ => Self::Closed,
        }
    }
}
