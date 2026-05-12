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
        }
    }
}
