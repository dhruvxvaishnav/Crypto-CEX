use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;
use uuid::Uuid;

use crate::auth::password::{hash_password, verify_password};
use crate::auth::tokens::{hash_refresh_token, TokenConfig, TokenPair};
use crate::clock::{SharedClock, SharedDelay, SharedIds};
use crate::repositories::{
    AccountStatus, CreateRefreshToken, CreateUser, RepositoryError, SharedAuthRepository,
};
use crate::schemas::{
    is_valid_email, normalize_email, password_policy_failures, LoginInput, SignupInput,
};

const DUPLICATE_EMAIL_FLOOR: Duration = Duration::from_millis(500);
const LOGIN_WINDOW_SECONDS: i64 = 15 * 60;
const MAX_LOGIN_FAILURES: u32 = 5;

/// Auth service.
#[derive(Clone)]
pub struct AuthService {
    repository: SharedAuthRepository,
    token_config: TokenConfig,
    clock: SharedClock,
    delay: SharedDelay,
    ids: SharedIds,
    attempts: Arc<Mutex<LoginAttempts>>,
}

/// Login response variant.
#[derive(Debug, Clone)]
pub enum LoginOutcome {
    /// Login produced access and refresh tokens.
    Tokens(TokenPair),
    /// Login requires MFA challenge completion.
    MfaRequired {
        /// Short-lived MFA token.
        mfa_token: String,
    },
}

/// Auth service error.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Invalid email address.
    #[error("invalid email")]
    InvalidEmail,
    /// Password failed policy.
    #[error("weak password")]
    WeakPassword {
        /// Missing policy requirements.
        missing: Vec<&'static str>,
    },
    /// Email already exists.
    #[error("email taken")]
    EmailTaken,
    /// Credentials are invalid.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// Login attempt limit exceeded.
    #[error("too many attempts")]
    TooManyAttempts,
    /// Refresh token is expired.
    #[error("refresh token expired")]
    TokenExpired,
    /// Refresh token is invalid.
    #[error("refresh token invalid")]
    TokenInvalid,
    /// Unexpected service failure.
    #[error("auth internal error")]
    Internal,
}

#[derive(Debug, Default)]
struct LoginAttempts {
    by_email: HashMap<String, FailureWindow>,
    by_ip: HashMap<String, FailureWindow>,
}

#[derive(Debug, Clone, Copy)]
struct FailureWindow {
    count: u32,
    first_failure_at: i64,
}

impl AuthService {
    /// Creates an auth service.
    #[must_use]
    pub fn new(
        repository: SharedAuthRepository,
        token_config: TokenConfig,
        clock: SharedClock,
        delay: SharedDelay,
        ids: SharedIds,
    ) -> Self {
        Self {
            repository,
            token_config,
            clock,
            delay,
            ids,
            attempts: Arc::new(Mutex::new(LoginAttempts::default())),
        }
    }

    /// Signs up a new user and returns tokens.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] for validation, uniqueness, token, or storage failures.
    pub async fn signup(&self, input: SignupInput) -> Result<TokenPair, AuthError> {
        let started = Instant::now();
        let email = normalised_email_or_error(&input.email)?;
        let missing = password_policy_failures(&input.password);
        if !missing.is_empty() {
            return Err(AuthError::WeakPassword { missing });
        }

        let password_hash = hash_password(&input.password).map_err(|_| AuthError::Internal)?;
        let user_id = self.ids.new_uuid();
        let user = self
            .repository
            .create_user(CreateUser {
                id: user_id,
                email,
                password_hash,
            })
            .await
            .map_err(|error| match error {
                RepositoryError::EmailTaken => AuthError::EmailTaken,
                RepositoryError::Unavailable => AuthError::Internal,
            });
        if matches!(user, Err(AuthError::EmailTaken)) {
            self.delay
                .sleep_until(started + DUPLICATE_EMAIL_FLOOR)
                .await;
        }
        let user = user?;
        self.mint_and_store_pair(
            user.id,
            self.ids.new_uuid(),
            self.ids.new_uuid(),
            user.kyc_level,
        )
        .await
    }

    /// Logs in with email and password.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] for invalid credentials, attempt limits, token, or storage failures.
    pub async fn login(
        &self,
        input: LoginInput,
        ip_key: String,
        user_agent: Option<String>,
    ) -> Result<LoginOutcome, AuthError> {
        let email = normalised_email_or_error(&input.email)?;
        let now = self.clock.now().unix_timestamp();
        self.ensure_login_allowed(&email, &ip_key, now).await?;

        let Some(user) = self
            .repository
            .find_user_by_email(&email)
            .await
            .map_err(|_| AuthError::Internal)?
        else {
            self.record_login_failure(&email, &ip_key, now).await;
            return Err(AuthError::InvalidCredentials);
        };
        if user.status != AccountStatus::Active {
            self.record_login_failure(&email, &ip_key, now).await;
            return Err(AuthError::InvalidCredentials);
        }
        if !verify_password(&input.password, &user.password_hash)
            .map_err(|_| AuthError::Internal)?
        {
            self.record_login_failure(&email, &ip_key, now).await;
            return Err(AuthError::InvalidCredentials);
        }

        self.clear_login_failures(&email, &ip_key).await;
        let session_id = self.ids.new_uuid();
        if user.totp_enabled {
            let mfa_token = self
                .token_config
                .mint_mfa_token(self.clock.now(), user.id, session_id, self.ids.new_uuid())
                .map_err(|_| AuthError::Internal)?;
            return Ok(LoginOutcome::MfaRequired { mfa_token });
        }

        let token_pair = self
            .mint_and_store_pair(user.id, session_id, self.ids.new_uuid(), user.kyc_level)
            .await?;
        tracing::info!(
            event = "auth.login.success",
            user_id = %user.id,
            ip = %ip_key,
            ua = user_agent.as_deref().unwrap_or("unknown")
        );
        Ok(LoginOutcome::Tokens(token_pair))
    }

    /// Rotates a refresh token family member.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] if the refresh token is invalid, expired, reused, or
    /// persistence fails.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        let token_hash = hash_refresh_token(refresh_token);
        let Some(stored) = self
            .repository
            .find_refresh_token(&token_hash)
            .await
            .map_err(|_| AuthError::Internal)?
        else {
            return Err(AuthError::TokenInvalid);
        };
        let now = self.clock.now();
        if stored.revoked_at.is_some() {
            self.repository
                .revoke_refresh_family(stored.family_id, now)
                .await
                .map_err(|_| AuthError::Internal)?;
            return Err(AuthError::TokenInvalid);
        }
        if stored.expires_at <= now {
            self.repository
                .revoke_refresh_token(stored.id, now)
                .await
                .map_err(|_| AuthError::Internal)?;
            return Err(AuthError::TokenExpired);
        }

        let Some(user) = self
            .repository
            .find_user_by_id(stored.user_id)
            .await
            .map_err(|_| AuthError::Internal)?
        else {
            return Err(AuthError::TokenInvalid);
        };
        if user.status != AccountStatus::Active {
            return Err(AuthError::TokenInvalid);
        }

        self.repository
            .revoke_refresh_token(stored.id, now)
            .await
            .map_err(|_| AuthError::Internal)?;
        self.mint_and_store_pair_with_family(
            user.id,
            stored.session_id,
            stored.family_id,
            self.ids.new_uuid(),
            user.kyc_level,
        )
        .await
    }

    async fn mint_and_store_pair(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        family_id: Uuid,
        kyc_level: i32,
    ) -> Result<TokenPair, AuthError> {
        self.mint_and_store_pair_with_family(
            user_id,
            session_id,
            family_id,
            self.ids.new_uuid(),
            kyc_level,
        )
        .await
    }

    async fn mint_and_store_pair_with_family(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        family_id: Uuid,
        token_id: Uuid,
        kyc_level: i32,
    ) -> Result<TokenPair, AuthError> {
        let pair = self
            .token_config
            .mint_pair(
                self.clock.now(),
                user_id,
                session_id,
                kyc_level,
                self.ids.new_uuid(),
            )
            .map_err(|_| AuthError::Internal)?;
        self.repository
            .insert_refresh_token(CreateRefreshToken {
                id: token_id,
                user_id,
                session_id,
                token_hash: pair.refresh_token_hash.clone(),
                family_id,
                expires_at: pair.refresh_expires_at,
            })
            .await
            .map_err(|_| AuthError::Internal)?;
        Ok(pair)
    }

    async fn ensure_login_allowed(
        &self,
        email: &str,
        ip_key: &str,
        now: i64,
    ) -> Result<(), AuthError> {
        let mut attempts = self.attempts.lock().await;
        attempts.prune(now);
        if attempts.is_locked(email, ip_key) {
            return Err(AuthError::TooManyAttempts);
        }
        drop(attempts);
        Ok(())
    }

    async fn record_login_failure(&self, email: &str, ip_key: &str, now: i64) {
        self.attempts.lock().await.record(email, ip_key, now);
        tracing::warn!(event = "auth.login.failure", reason = "invalid_credentials");
    }

    async fn clear_login_failures(&self, email: &str, ip_key: &str) {
        let mut attempts = self.attempts.lock().await;
        attempts.by_email.remove(email);
        attempts.by_ip.remove(ip_key);
    }
}

impl LoginAttempts {
    fn record(&mut self, email: &str, ip_key: &str, now: i64) {
        increment_window(
            self.by_email
                .entry(email.to_owned())
                .or_insert(FailureWindow {
                    count: 0,
                    first_failure_at: now,
                }),
        );
        increment_window(
            self.by_ip
                .entry(ip_key.to_owned())
                .or_insert(FailureWindow {
                    count: 0,
                    first_failure_at: now,
                }),
        );
    }

    fn prune(&mut self, now: i64) {
        self.by_email
            .retain(|_, window| now - window.first_failure_at <= LOGIN_WINDOW_SECONDS);
        self.by_ip
            .retain(|_, window| now - window.first_failure_at <= LOGIN_WINDOW_SECONDS);
    }

    fn is_locked(&self, email: &str, ip_key: &str) -> bool {
        self.by_email
            .get(email)
            .is_some_and(|window| window.count >= MAX_LOGIN_FAILURES)
            || self
                .by_ip
                .get(ip_key)
                .is_some_and(|window| window.count >= MAX_LOGIN_FAILURES)
    }
}

fn increment_window(window: &mut FailureWindow) {
    window.count = window.count.saturating_add(1);
}

fn normalised_email_or_error(email: &str) -> Result<String, AuthError> {
    let email = normalize_email(email);
    if is_valid_email(&email) {
        Ok(email)
    } else {
        Err(AuthError::InvalidEmail)
    }
}
