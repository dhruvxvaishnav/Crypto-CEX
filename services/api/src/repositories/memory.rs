use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    AccountStatus, AuthRepository, AuthUser, CreateRefreshToken, CreateUser, RefreshToken,
    RepositoryError,
};

/// In-memory auth repository for tests.
#[derive(Debug, Default)]
pub struct MemoryAuthRepository {
    inner: Mutex<MemoryInner>,
}

#[derive(Debug, Default)]
struct MemoryInner {
    users_by_id: HashMap<Uuid, AuthUser>,
    user_id_by_email: HashMap<String, Uuid>,
    refresh_by_hash: HashMap<String, RefreshToken>,
}

impl MemoryAuthRepository {
    /// Creates a shared memory repository.
    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Inserts a user fixture.
    pub async fn insert_user(&self, user: AuthUser) {
        let mut inner = self.inner.lock().await;
        inner.user_id_by_email.insert(user.email.clone(), user.id);
        inner.users_by_id.insert(user.id, user);
        drop(inner);
    }
}

#[async_trait]
impl AuthRepository for MemoryAuthRepository {
    async fn create_user(&self, input: CreateUser) -> Result<AuthUser, RepositoryError> {
        let mut inner = self.inner.lock().await;
        if inner.user_id_by_email.contains_key(&input.email) {
            return Err(RepositoryError::EmailTaken);
        }
        let user = AuthUser {
            id: input.id,
            email: input.email,
            password_hash: input.password_hash,
            status: AccountStatus::Active,
            kyc_level: 0,
            totp_enabled: false,
        };
        inner.user_id_by_email.insert(user.email.clone(), user.id);
        inner.users_by_id.insert(user.id, user.clone());
        drop(inner);
        Ok(user)
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<AuthUser>, RepositoryError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .user_id_by_email
            .get(email)
            .and_then(|id| inner.users_by_id.get(id))
            .cloned())
    }

    async fn insert_refresh_token(&self, input: CreateRefreshToken) -> Result<(), RepositoryError> {
        let mut inner = self.inner.lock().await;
        inner.refresh_by_hash.insert(
            input.token_hash.clone(),
            RefreshToken {
                id: input.id,
                user_id: input.user_id,
                session_id: input.session_id,
                token_hash: input.token_hash,
                family_id: input.family_id,
                expires_at: input.expires_at,
                revoked_at: None,
            },
        );
        drop(inner);
        Ok(())
    }

    async fn find_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshToken>, RepositoryError> {
        Ok(self
            .inner
            .lock()
            .await
            .refresh_by_hash
            .get(token_hash)
            .cloned())
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<AuthUser>, RepositoryError> {
        Ok(self.inner.lock().await.users_by_id.get(&user_id).cloned())
    }

    async fn revoke_refresh_token(
        &self,
        token_id: Uuid,
        revoked_at: OffsetDateTime,
    ) -> Result<(), RepositoryError> {
        let mut inner = self.inner.lock().await;
        for token in inner.refresh_by_hash.values_mut() {
            if token.id == token_id && token.revoked_at.is_none() {
                token.revoked_at = Some(revoked_at);
            }
        }
        drop(inner);
        Ok(())
    }

    async fn revoke_refresh_family(
        &self,
        family_id: Uuid,
        revoked_at: OffsetDateTime,
    ) -> Result<(), RepositoryError> {
        let mut inner = self.inner.lock().await;
        for token in inner.refresh_by_hash.values_mut() {
            if token.family_id == family_id && token.revoked_at.is_none() {
                token.revoked_at = Some(revoked_at);
            }
        }
        drop(inner);
        Ok(())
    }
}
