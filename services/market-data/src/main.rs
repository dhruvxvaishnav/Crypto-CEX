use anyhow::Context;
use cex_market_data::binance::combined_book_ticker_url;
use cex_market_data::Config;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing_subscriber::EnvFilter;

const DB_MAX_CONNECTIONS: u32 = 5;

#[derive(Debug, sqlx::FromRow)]
struct MarketRow {
    symbol: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialising tracing subscriber: {error}"))?;

    let config = Config::from_env().context("loading market-data config")?;
    let pool = PgPoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await
        .context("connecting to postgres")?;
    let markets = load_active_markets(&pool)
        .await
        .context("loading active markets")?;
    let symbols = markets
        .into_iter()
        .map(|market| market.symbol)
        .collect::<Vec<_>>();
    let stream_url = combined_book_ticker_url(&symbols);

    tracing::info!(
        market_count = symbols.len(),
        stream_url,
        "market_data.worker.configured"
    );
    run(config).await
}

async fn run(config: Config) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(config.quote_refresh_interval);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("market_data.quote_refresh.tick");
            }
            signal = tokio::signal::ctrl_c() => {
                if let Err(error) = signal {
                    tracing::error!(error = %error, "market_data.shutdown.signal_failed");
                }
                break;
            }
        }
    }
    Ok(())
}

async fn load_active_markets(pool: &PgPool) -> Result<Vec<MarketRow>, sqlx::Error> {
    sqlx::query_as::<_, MarketRow>(
        r"
        SELECT symbol
        FROM markets
        WHERE status = 'trading'
        ORDER BY symbol
        ",
    )
    .fetch_all(pool)
    .await
}
