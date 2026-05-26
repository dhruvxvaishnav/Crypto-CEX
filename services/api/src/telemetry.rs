//! OpenTelemetry SDK initialisation for the API service.
//!
//! Wires a `TracerProvider` (OTLP gRPC → collector) and an `SdkMeterProvider`
//! (same endpoint, 15-second push interval) and bridges them to the `tracing`
//! subscriber so that every `tracing::span!` macro call emits an OpenTelemetry span.
//!
//! PRD §19.3 — One trace per HTTP request; spans: validate → auth → engine → response.

use std::time::Duration;

use anyhow::Context as _;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    runtime::Tokio,
    trace::{BatchSpanProcessor, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

const METRICS_PUSH_INTERVAL_SECS: u64 = 15;

/// RAII guard — shuts down OpenTelemetry providers on drop, flushing pending data.
pub struct OtelGuard {
    tracer_provider: TracerProvider,
    meter_provider: SdkMeterProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("OTel tracer shutdown error: {e:?}");
        }
        if let Err(e) = self.meter_provider.shutdown() {
            eprintln!("OTel meter shutdown error: {e:?}");
        }
    }
}

/// Initialises tracing subscriber wired to an OTLP exporter.
///
/// `otlp_endpoint` should be the gRPC address of an OpenTelemetry collector,
/// e.g. `http://otelcol:4317`. When `None`, falls back to `http://127.0.0.1:4317`.
/// The exporter connects lazily — the function succeeds even if the collector
/// is not yet running.
///
/// # Errors
///
/// Returns an error if the OTLP exporters cannot be built or a global
/// subscriber is already installed.
pub fn init_telemetry(
    service: &'static str,
    otlp_endpoint: Option<&str>,
) -> anyhow::Result<OtelGuard> {
    let resource = Resource::new(vec![KeyValue::new(SERVICE_NAME, service)]);
    let endpoint = otlp_endpoint.unwrap_or("http://127.0.0.1:4317");

    let span_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .context("building OTLP span exporter")?;

    let tracer_provider = TracerProvider::builder()
        .with_resource(resource.clone())
        .with_span_processor(BatchSpanProcessor::builder(span_exporter, Tokio).build())
        .build();
    global::set_tracer_provider(tracer_provider.clone());

    let metrics_exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .context("building OTLP metrics exporter")?;
    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(
            PeriodicReader::builder(metrics_exporter, Tokio)
                .with_interval(Duration::from_secs(METRICS_PUSH_INTERVAL_SECS))
                .build(),
        )
        .build();
    global::set_meter_provider(meter_provider.clone());

    // W3C TraceContext propagation so incoming `traceparent` headers are honoured.
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = tracer_provider.tracer(service);
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .with(OpenTelemetryLayer::new(tracer))
        .try_init()
        .map_err(|e| anyhow::anyhow!("initialising tracing subscriber: {e}"))?;

    Ok(OtelGuard {
        tracer_provider,
        meter_provider,
    })
}
