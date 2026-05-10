use axum::extract::Request;
use axum::http::{header::HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tokio::time::Instant;
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

/// Ensures every request and response has a request ID.
pub async fn request_id(mut request: Request, next: Next) -> Response {
    let started = Instant::now();
    let request_id = request
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::new_v4);
    request
        .extensions_mut()
        .insert(RequestContext { request_id });

    let method = request.method().clone();
    let route = request.uri().path().to_owned();
    tracing::info!(
        event = "http.request",
        %method,
        route = %route,
        request_id = %request_id
    );
    let mut response = next.run(request).await;
    let status = response.status().as_u16();
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        event = "http.response",
        %method,
        route = %route,
        request_id = %request_id,
        status,
        duration_ms
    );
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
