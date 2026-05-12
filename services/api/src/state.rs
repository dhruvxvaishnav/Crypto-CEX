use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::tokens::TokenConfig;
use crate::auth::AuthService;
use crate::engine_client::EngineClient;
use crate::readiness::SharedReadiness;
use crate::ws::hub::Hub;

/// Application state shared by all handlers.
///
/// Every field is either `Clone`-cheap (Arc, pool clone) or `Copy`, so axum
/// can inject it into handlers without allocation.
#[derive(Clone)]
pub struct AppState {
    /// Auth service (signup, login, refresh).
    pub auth: AuthService,
    /// JWT config — used by the auth extractor for token verification (~1 μs).
    pub token_config: Arc<TokenConfig>,
    /// Readiness dependency checker.
    pub readiness: SharedReadiness,
    /// Postgres connection pool for DB queries.
    pub db: PgPool,
    /// Persistent engine client (multiplexed, reconnecting).
    pub engine: EngineClient,
    /// WebSocket subscription hub.
    pub hub: Hub,
    /// Redis connection manager for rate limiting and replay protection.
    /// `None` in test environments where Redis is not available.
    pub redis: Option<redis::aio::ConnectionManager>,
    /// pgcrypto symmetric key for TOTP and API-key secret encryption.
    pub pgcrypto_key: Arc<str>,
    /// Whether the faucet endpoint is enabled (false in production by default).
    pub faucet_enabled: bool,
}

impl AppState {
    /// Creates application state.
    #[must_use]
    pub fn new(
        auth: AuthService,
        token_config: Arc<TokenConfig>,
        readiness: SharedReadiness,
        db: PgPool,
        engine: EngineClient,
        hub: Hub,
    ) -> Self {
        Self {
            auth,
            token_config,
            readiness,
            db,
            engine,
            hub,
            redis: None,
            pgcrypto_key: Arc::from("dev-insecure-key"),
            faucet_enabled: true,
        }
    }

    /// Attaches a live Redis connection manager.
    #[must_use]
    pub fn with_redis(mut self, conn: redis::aio::ConnectionManager) -> Self {
        self.redis = Some(conn);
        self
    }

    /// Attaches the pgcrypto encryption key.
    #[must_use]
    pub fn with_pgcrypto_key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.pgcrypto_key = key.into();
        self
    }

    /// Sets the faucet enabled flag.
    #[must_use]
    pub fn with_faucet_enabled(mut self, enabled: bool) -> Self {
        self.faucet_enabled = enabled;
        self
    }
}
