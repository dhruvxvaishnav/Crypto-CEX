use std::time::Duration;

use anyhow::Context as _;
use cex_settlement::{Config, EngineEventBridge, PostgresSettlement};
use opentelemetry::metrics::{Counter, Gauge};
use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    runtime::Tokio,
    trace::{BatchSpanProcessor, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use sqlx::postgres::{PgListener, PgPool, PgPoolOptions};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

const DB_MAX_CONNECTIONS: u32 = 5;
const ENGINE_EVENTS_CHANNEL: &str = "engine_events";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env().context("loading settlement config")?;

    // OTel guard held for process lifetime.
    let _otel_guard = init_telemetry("cex-settlement", config.otlp_endpoint.as_deref())
        .context("init telemetry")?;

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

    // PRD §19.2 metrics.
    let meter = global::meter("cex-settlement");
    let processed_counter = meter
        .u64_counter("settlement_events_processed_total")
        .with_description("Total engine events processed by the settlement worker")
        .build();
    let lag_gauge = meter
        .i64_gauge("settlement_lag_seq")
        .with_description("Engine sequence minus last processed settlement sequence")
        .build();

    let settlement = PostgresSettlement::new(pool.clone());
    let bridge = EngineEventBridge::new(pool.clone(), config.engine_addr);
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
    let result = run(
        settlement,
        pool,
        listener,
        config.poll_interval,
        processed_counter,
        lag_gauge,
    )
    .await;
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
    pool: PgPool,
    mut listener: PgListener,
    poll_interval: Duration,
    processed_counter: Counter<u64>,
    lag_gauge: Gauge<i64>,
) -> anyhow::Result<()> {
    loop {
        let processed = settlement
            .process_available()
            .await
            .context("processing settlement events")?;
        if processed > 0 {
            processed_counter.add(processed, &[]);
            // Update lag after each processing batch.
            let lag = settlement_lag(&pool).await;
            lag_gauge.record(lag, &[KeyValue::new("worker", "settlement")]);
            tracing::info!(processed, lag, "settlement.batch.processed");
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

/// Queries the number of engine events that have not yet been settled.
async fn settlement_lag(pool: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE((SELECT MAX(seq) FROM engine_events), 0) \
         - COALESCE((SELECT last_processed_seq FROM worker_state \
                     WHERE worker_name = 'settlement'), 0)",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

/// Initialises tracing with an OTLP exporter (traces only, no metrics push).
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
