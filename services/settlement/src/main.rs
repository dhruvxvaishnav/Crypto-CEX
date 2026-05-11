use std::time::Duration;

use anyhow::Context;
use cex_settlement::{Config, EngineEventBridge, PostgresSettlement};
use sqlx::postgres::{PgListener, PgPoolOptions};
use tracing_subscriber::EnvFilter;

const DB_MAX_CONNECTIONS: u32 = 5;
const ENGINE_EVENTS_CHANNEL: &str = "engine_events";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialising tracing subscriber: {error}"))?;

    let config = Config::from_env().context("loading settlement config")?;
    let pool = PgPoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await
        .context("connecting to postgres")?;
    let mut listener = PgListener::connect_with(&pool)
        .await
        .context("connecting postgres listener")?;
    listener
        .listen(ENGINE_EVENTS_CHANNEL)
        .await
        .context("listening for engine event notifications")?;

    let settlement = PostgresSettlement::new(pool.clone());
    let bridge = EngineEventBridge::new(pool, config.engine_addr);
    let bridge_reconnect_interval = config.bridge_reconnect_interval;
    let bridge_task = tokio::spawn(async move {
        loop {
            if let Err(error) = bridge.run_once().await {
                tracing::warn!(error = %error, "settlement.bridge.reconnecting");
            }
            tokio::time::sleep(bridge_reconnect_interval).await;
        }
    });

    tracing::info!("settlement.worker.started");
    let result = run(settlement, listener, config.poll_interval).await;
    bridge_task.abort();
    if let Err(error) = bridge_task.await {
        if !error.is_cancelled() {
            return Err(error).context("joining engine event bridge task");
        }
    }
    result
}

async fn run(
    settlement: PostgresSettlement,
    mut listener: PgListener,
    poll_interval: Duration,
) -> anyhow::Result<()> {
    loop {
        let processed = settlement
            .process_available()
            .await
            .context("processing settlement events")?;
        if processed > 0 {
            tracing::info!(processed, "settlement.batch.processed");
        }

        tokio::select! {
            notification = listener.recv() => {
                let notification = notification.context("receiving engine event notification")?;
                tracing::debug!(payload = notification.payload(), "settlement.notification.received");
            }
            () = tokio::time::sleep(poll_interval) => {}
            signal = tokio::signal::ctrl_c() => {
                if let Err(error) = signal {
                    tracing::error!(error = %error, "settlement.shutdown.signal_failed");
                }
                break;
            }
        }
    }
    Ok(())
}
