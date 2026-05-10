use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use cex_api::auth::tokens::TokenConfig;
use cex_api::auth::AuthService;
use cex_api::clock::{SystemClock, TokioDelay, UuidSource};
use cex_api::config::Config;
use cex_api::engine_client::EngineClient;
use cex_api::readiness::PgEngineReadiness;
use cex_api::repositories::postgres::PgAuthRepository;
use cex_api::{build_router, AppState};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DB_MAX_CONNECTIONS: u32 = 10;
const ENGINE_TIMEOUT: Duration = Duration::from_millis(50);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialising tracing subscriber: {error}"))?;

    let config = Config::from_env().context("loading api config")?;
    let pool = PgPoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await
        .context("connecting to postgres")?;
    let engine_client = EngineClient::new(config.engine_addr, ENGINE_TIMEOUT);
    let auth_repository = Arc::new(PgAuthRepository::new(pool.clone()));
    let token_config = TokenConfig {
        jwt_secret: config.jwt_secret,
        access_token_ttl: config.access_token_ttl,
        mfa_token_ttl: config.mfa_token_ttl,
        refresh_token_ttl: config.refresh_token_ttl,
    };
    let auth = AuthService::new(
        auth_repository,
        token_config,
        Arc::new(SystemClock),
        Arc::new(TokioDelay),
        Arc::new(UuidSource),
    );
    let readiness = Arc::new(PgEngineReadiness::new(pool, engine_client));
    let app = build_router(AppState::new(auth, readiness));
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .context("binding api listener")?;

    tracing::info!(addr = %config.bind_addr, "api.server.listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving api")
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %error, "api.shutdown.signal_failed");
    }
}
