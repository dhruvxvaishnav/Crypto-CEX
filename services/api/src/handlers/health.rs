use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::Json;

use crate::errors::{ApiError, ErrorCode};
use crate::middleware::RequestContext;
use crate::schemas::{HealthResponse, ReadyResponse};
use crate::state::AppState;

/// Liveness endpoint.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Readiness endpoint.
///
/// # Errors
///
/// Returns [`ApiError`] when Postgres or the matching engine is unavailable.
pub async fn ready(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
) -> Result<Json<ReadyResponse>, ApiError> {
    state
        .readiness
        .check(context.request_id)
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::EngineTimeout,
                "Service dependencies are not ready",
            )
            .with_request_id(context.request_id)
        })?;
    Ok(Json(ReadyResponse { status: "ready" }))
}
