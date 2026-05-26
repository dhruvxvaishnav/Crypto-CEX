use std::collections::BTreeSet;

use anyhow::Context as _;
use cex_market_data::binance::{default_top_usdt_symbols, run_reconnecting_stream, BinanceEvent};
use cex_market_data::engine::EngineClient;
use cex_market_data::market_maker::MarketMaker;
use cex_market_data::persistence::MarketDataRepository;
use cex_market_data::Config;
use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    runtime::Tokio,
    trace::{BatchSpanProcessor, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::mpsc;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

const BINANCE_EVENT_CAPACITY: usize = 4_096;
const DB_MAX_CONNECTIONS: u32 = 5;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env().context("loading market-data config")?;

    let _otel_guard = init_telemetry("cex-market-data", config.otlp_endpoint.as_deref())
        .context("init telemetry")?;

    let pool = PgPoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await
        .context("connecting to postgres")?;
    let repository = MarketDataRepository::new(pool);
    let markets = repository
        .load_active_markets()
        .await
        .context("loading active markets")?;
    let local_symbols = markets
        .into_iter()
        .map(|market| market.symbol)
        .collect::<Vec<_>>();
    let stream_symbols = stream_symbols(&local_symbols);
    let market_maker_user_id = repository
        .market_maker_user_id()
        .await
        .context("loading market-maker user")?;
    let engine = EngineClient::new(config.engine_addr, config.engine_timeout);
    let market_maker = MarketMaker::new(
        engine,
        market_maker_user_id,
        config.stale_after,
        local_symbols.clone(),
    );

    tracing::info!(
        local_market_count = local_symbols.len(),
        stream_symbol_count = stream_symbols.len(),
        "market_data.worker.started"
    );
    run(repository, market_maker, stream_symbols, config).await
}

async fn run(
    repository: MarketDataRepository,
    mut market_maker: MarketMaker,
    symbols: Vec<String>,
    config: Config,
) -> anyhow::Result<()> {
    let (event_tx, mut event_rx) = mpsc::channel(BINANCE_EVENT_CAPACITY);
    let stream_task = tokio::spawn(run_reconnecting_stream(symbols, event_tx));
    let mut quote_interval = tokio::time::interval(config.quote_refresh_interval);

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                let Some(event) = event else {
                    break;
                };
                handle_event(&repository, &mut market_maker, event).await?;
            }
            _ = quote_interval.tick() => {
                market_maker
                    .refresh_quotes()
                    .await
                    .context("refreshing market-maker quotes")?;
            }
            signal = tokio::signal::ctrl_c() => {
                if let Err(error) = signal {
                    tracing::error!(error = %error, "market_data.shutdown.signal_failed");
                }
                break;
            }
        }
    }

    stream_task.abort();
    if let Err(error) = stream_task.await {
        if !error.is_cancelled() {
            return Err(error).context("joining binance stream task");
        }
    }
    Ok(())
}

async fn handle_event(
    repository: &MarketDataRepository,
    market_maker: &mut MarketMaker,
    event: BinanceEvent,
) -> anyhow::Result<()> {
    match event {
        BinanceEvent::BookTicker(ticker) => {
            market_maker.record_ticker(ticker);
        }
        BinanceEvent::Kline(kline) => {
            repository
                .upsert_kline(&kline)
                .await
                .context("persisting kline")?;
        }
    }
    Ok(())
}

fn stream_symbols(local_symbols: &[String]) -> Vec<String> {
    let mut symbols = default_top_usdt_symbols()
        .into_iter()
        .collect::<BTreeSet<_>>();
    symbols.extend(local_symbols.iter().cloned());
    symbols.into_iter().collect()
}

fn init_telemetry(
    service: &'static str,
    otlp_endpoint: Option<&str>,
) -> anyhow::Result<TracerProvider> {
    let resource = Resource::new(vec![KeyValue::new(SERVICE_NAME, service)]);
    let endpoint = otlp_endpoint.unwrap_or("http://127.0.0.1:4317");

    let span_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .context("building OTLP span exporter")?;

    let tracer_provider = TracerProvider::builder()
        .with_resource(resource)
        .with_span_processor(BatchSpanProcessor::builder(span_exporter, Tokio).build())
        .build();

    global::set_tracer_provider(tracer_provider.clone());
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = tracer_provider.tracer(service);
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .with(OpenTelemetryLayer::new(tracer))
        .try_init()
        .map_err(|e| anyhow::anyhow!("initialising tracing subscriber: {e}"))?;

    Ok(tracer_provider)
}
