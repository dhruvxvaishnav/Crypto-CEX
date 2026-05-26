use std::sync::OnceLock;

use axum::extract::Request;
use axum::http::{header::HeaderName, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::propagation::Extractor;
use opentelemetry::{global, KeyValue};
use tokio::time::Instant;
use tracing::Instrument as _;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use uuid::Uuid;

use crate::errors::{ErrorBody, ErrorCode, ErrorEnvelope};

/// Request header carrying the request ID.
pub const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Per-request context inserted by request-id middleware.
#[derive(Debug, Clone, Copy)]
pub struct RequestContext {
    /// Request correlation ID.
    pub request_id: Uuid,
}

// ── OTel metric instruments (created once, reused per request) ────────────────

static HTTP_REQUESTS_TOTAL: OnceLock<Counter<u64>> = OnceLock::new();
static HTTP_DURATION_SECONDS: OnceLock<Histogram<f64>> = OnceLock::new();

fn http_requests_total() -> &'static Counter<u64> {
    HTTP_REQUESTS_TOTAL.get_or_init(|| {
        global::meter("cex-api")
            .u64_counter("http_requests_total")
            .with_description("Total HTTP requests by method, route, and status code")
            .build()
    })
}

fn http_duration_seconds() -> &'static Histogram<f64> {
    HTTP_DURATION_SECONDS.get_or_init(|| {
        global::meter("cex-api")
            .f64_histogram("http_request_duration_seconds")
            .with_description("HTTP request duration in seconds")
            .build()
    })
}

// ── W3C TraceContext extraction from incoming HTTP headers ────────────────────

/// Adapter so `TextMapPropagator::extract` can read from a [`HeaderMap`].
struct HeaderCarrier<'a>(&'a HeaderMap);

impl Extractor for HeaderCarrier<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(HeaderName::as_str).collect()
    }
}

// ── Middleware ────────────────────────────────────────────────────────────────

/// Request-id, structured logging, OpenTelemetry span, and HTTP metrics middleware.
///
/// Assigns `X-Request-Id`, creates a per-request span (PRD §19.3), and records
/// `http_requests_total` / `http_request_duration_seconds` (PRD §19.2).
pub async fn request_id(mut request: Request, next: Next) -> Response {
    let started = Instant::now();

    let request_id = request
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok())
        .unwrap_or_else(Uuid::new_v4);

    let method = request.method().as_str().to_owned();
    let route = request.uri().path().to_owned();

    // Extract W3C traceparent from incoming headers to link to upstream trace.
    let parent_cx =
        global::get_text_map_propagator(|prop| prop.extract(&HeaderCarrier(request.headers())));

    // One OTel span per request — child spans (auth, engine, db) nest inside it.
    let span = tracing::info_span!(
        "http.request",
        http.request.method = %method,
        url.path = %route,
        request_id = %request_id,
        http.response.status_code = tracing::field::Empty,
    );
    span.set_parent(parent_cx);

    request
        .extensions_mut()
        .insert(RequestContext { request_id });

    tracing::info!(
        parent: &span,
        event = "http.request",
        method = %method,
        route = %route,
        request_id = %request_id,
    );

    // Run inner middleware/handler inside the span so child spans nest correctly.
    let response = next.run(request).instrument(span.clone()).await;

    let status = response.status().as_u16();
    let duration_secs = started.elapsed().as_secs_f64();
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    span.record("http.response.status_code", status);
    tracing::info!(
        parent: &span,
        event = "http.response",
        method = %method,
        route = %route,
        request_id = %request_id,
        status,
        duration_ms,
    );

    // PRD §19.2 metrics.
    let labels = [
        KeyValue::new("method", method.clone()),
        KeyValue::new("route", route.clone()),
        KeyValue::new("status", status.to_string()),
    ];
    http_requests_total().add(1, &labels);
    http_duration_seconds().record(
        duration_secs,
        &[
            KeyValue::new("method", method),
            KeyValue::new("route", route),
        ],
    );

    let mut response = response;
    if let Ok(header_value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(X_REQUEST_ID, header_value);
    }
    response
}

/// Fallback for unmatched routes using the standard envelope.
pub async fn not_found(
    axum::extract::Extension(context): axum::extract::Extension<RequestContext>,
) -> (StatusCode, axum::Json<ErrorEnvelope>) {
    error_json(
        StatusCode::NOT_FOUND,
        ErrorCode::NotFound,
        "Route not found",
        context.request_id,
    )
}

fn error_json(
    status: StatusCode,
    code: ErrorCode,
    message: impl Into<String>,
    request_id: Uuid,
) -> (StatusCode, axum::Json<ErrorEnvelope>) {
    (
        status,
        axum::Json(ErrorEnvelope {
            error: ErrorBody {
                code,
                message: message.into(),
                details: None,
            },
            request_id,
        }),
    )
}
