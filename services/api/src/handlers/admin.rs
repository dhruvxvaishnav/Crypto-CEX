use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cex_proto::{CancelAllRequest, MarketControlRequest, SnapshotRequest};
use serde::Serialize;
use uuid::Uuid;

use crate::engine_client::EngineClientError;
use crate::errors::{ApiError, ErrorCode};
use crate::extractors::auth::AuthenticatedUser;
use crate::middleware::RequestContext;
use crate::repositories::admin as repo;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminActionResponse {
    symbol: Option<String>,
    status: Option<String>,
    canceled_order_ids: Vec<Uuid>,
    count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineStateResponse {
    markets: Vec<EngineMarketStateResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineMarketStateResponse {
    symbol: String,
    status: String,
    open_order_count: i64,
    best_bid: Option<[String; 2]>,
    best_ask: Option<[String; 2]>,
    seq: Option<u64>,
}

/// `POST /admin/markets/:symbol/halt`
///
/// # Errors
///
/// Returns [`ApiError`] when the caller is not an admin, the market is unknown,
/// or the engine/database operation fails.
pub async fn halt_market(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(symbol): Path<String>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = ensure_admin(&state, caller.user_id, ctx.request_id).await?;
    let symbol = symbol.to_uppercase();
    let market = find_market(&state, &symbol, ctx.request_id).await?;
    let ack = state
        .engine
        .halt_market(MarketControlRequest {
            request_id: ctx.request_id,
            symbol: symbol.clone(),
        })
        .await
        .map_err(|err| engine_error(&err, ctx.request_id))?;
    repo::update_market_status(&state.db, &symbol, "halted")
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "admin.market_halt.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    audit(
        &state,
        admin.id,
        "market.halt",
        "market",
        Some(market.id),
        serde_json::json!({ "symbol": symbol, "seq": ack.seq }),
        ctx.request_id,
    )
    .await?;

    Ok(Json(AdminActionResponse {
        symbol: Some(symbol),
        status: Some("halted".to_owned()),
        canceled_order_ids: Vec::new(),
        count: 0,
    }))
}

/// `POST /admin/markets/:symbol/resume`
///
/// # Errors
///
/// Returns [`ApiError`] when the caller is not an admin, the market is unknown,
/// or the engine/database operation fails.
pub async fn resume_market(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(symbol): Path<String>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = ensure_admin(&state, caller.user_id, ctx.request_id).await?;
    let symbol = symbol.to_uppercase();
    let market = find_market(&state, &symbol, ctx.request_id).await?;
    let ack = state
        .engine
        .resume_market(MarketControlRequest {
            request_id: ctx.request_id,
            symbol: symbol.clone(),
        })
        .await
        .map_err(|err| engine_error(&err, ctx.request_id))?;
    repo::update_market_status(&state.db, &symbol, "trading")
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "admin.market_resume.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    audit(
        &state,
        admin.id,
        "market.resume",
        "market",
        Some(market.id),
        serde_json::json!({ "symbol": symbol, "seq": ack.seq }),
        ctx.request_id,
    )
    .await?;

    Ok(Json(AdminActionResponse {
        symbol: Some(symbol),
        status: Some("trading".to_owned()),
        canceled_order_ids: Vec::new(),
        count: 0,
    }))
}

/// `POST /admin/markets/:symbol/cancel-all`
///
/// # Errors
///
/// Returns [`ApiError`] when the caller is not an admin, the market is unknown,
/// or the engine/database operation fails.
pub async fn cancel_all_in_market(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(symbol): Path<String>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = ensure_admin(&state, caller.user_id, ctx.request_id).await?;
    let symbol = symbol.to_uppercase();
    let market = find_market(&state, &symbol, ctx.request_id).await?;
    let users = repo::users_with_open_orders_in_market(&state.db, &symbol)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "admin.market_cancel_all.users_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    let canceled = cancel_for_users(&state, &users, Some(symbol.clone()), ctx.request_id).await?;
    repo::mark_orders_canceled(&state.db, &canceled)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "admin.market_cancel_all.mark_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    audit(
        &state,
        admin.id,
        "market.cancel_all",
        "market",
        Some(market.id),
        serde_json::json!({ "symbol": symbol, "count": canceled.len() }),
        ctx.request_id,
    )
    .await?;

    Ok(Json(AdminActionResponse {
        symbol: Some(symbol),
        status: None,
        count: canceled.len(),
        canceled_order_ids: canceled,
    }))
}

/// `POST /admin/users/:id/freeze`
///
/// # Errors
///
/// Returns [`ApiError`] when the caller is not an admin, the user is unknown,
/// or the engine/database operation fails.
pub async fn freeze_user(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = ensure_admin(&state, caller.user_id, ctx.request_id).await?;
    let updated = repo::freeze_user(&state.db, user_id).await.map_err(|err| {
        tracing::error!(err = %err, request_id = %ctx.request_id, "admin.user_freeze.db_error");
        ApiError::internal().with_request_id(ctx.request_id)
    })?;
    if !updated {
        return Err(not_found("User not found", ctx.request_id));
    }
    let canceled = cancel_for_users(&state, &[user_id], None, ctx.request_id).await?;
    repo::mark_orders_canceled(&state.db, &canceled)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "admin.user_freeze.mark_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;
    audit(
        &state,
        admin.id,
        "user.freeze",
        "user",
        Some(user_id),
        serde_json::json!({ "canceledOrderCount": canceled.len() }),
        ctx.request_id,
    )
    .await?;

    Ok(Json(AdminActionResponse {
        symbol: None,
        status: Some("frozen".to_owned()),
        count: canceled.len(),
        canceled_order_ids: canceled,
    }))
}

/// `GET /admin/engine/state`
///
/// # Errors
///
/// Returns [`ApiError`] when the caller is not an admin or state cannot be read.
pub async fn engine_state(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_admin(&state, caller.user_id, ctx.request_id).await?;
    let rows = repo::market_state(&state.db).await.map_err(|err| {
        tracing::error!(err = %err, request_id = %ctx.request_id, "admin.engine_state.db_error");
        ApiError::internal().with_request_id(ctx.request_id)
    })?;

    let mut markets = Vec::with_capacity(rows.len());
    for row in rows {
        let snapshot = state
            .engine
            .snapshot(SnapshotRequest {
                request_id: ctx.request_id,
                symbol: row.symbol.clone(),
                depth: 1,
            })
            .await
            .ok();
        markets.push(EngineMarketStateResponse {
            symbol: row.symbol,
            status: row.status,
            open_order_count: row.open_order_count,
            best_bid: snapshot
                .as_ref()
                .and_then(|snap| snap.bids.first())
                .map(|(price, qty)| [price.to_string(), qty.to_string()]),
            best_ask: snapshot
                .as_ref()
                .and_then(|snap| snap.asks.first())
                .map(|(price, qty)| [price.to_string(), qty.to_string()]),
            seq: snapshot.map(|snap| snap.seq),
        });
    }

    Ok(Json(EngineStateResponse { markets }))
}

async fn ensure_admin(
    state: &AppState,
    user_id: Uuid,
    request_id: Uuid,
) -> Result<repo::AdminUserRow, ApiError> {
    let Some(row) = repo::find_admin_user(&state.db, user_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %request_id, "admin.auth.db_error");
            ApiError::internal().with_request_id(request_id)
        })?
    else {
        return Err(forbidden(request_id));
    };
    if !row.is_admin || row.status != "active" {
        return Err(forbidden(request_id));
    }
    Ok(row)
}

async fn find_market(
    state: &AppState,
    symbol: &str,
    request_id: Uuid,
) -> Result<repo::AdminMarketRow, ApiError> {
    repo::find_market(&state.db, symbol)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %request_id, "admin.market.db_error");
            ApiError::internal().with_request_id(request_id)
        })?
        .ok_or_else(|| not_found("Market not found", request_id))
}

async fn cancel_for_users(
    state: &AppState,
    users: &[Uuid],
    symbol: Option<String>,
    request_id: Uuid,
) -> Result<Vec<Uuid>, ApiError> {
    let mut canceled = Vec::new();
    for user_id in users {
        let ack = state
            .engine
            .cancel_all(CancelAllRequest {
                request_id,
                user_id: *user_id,
                symbol: symbol.clone(),
            })
            .await
            .map_err(|err| engine_error(&err, request_id))?;
        canceled.extend(ack.order_ids);
    }
    Ok(canceled)
}

async fn audit(
    state: &AppState,
    actor_user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    metadata: serde_json::Value,
    request_id: Uuid,
) -> Result<(), ApiError> {
    repo::insert_audit_log(
        &state.db,
        actor_user_id,
        action,
        target_type,
        target_id,
        metadata,
    )
    .await
    .map_err(|err| {
        tracing::error!(err = %err, request_id = %request_id, "admin.audit.db_error");
        ApiError::internal().with_request_id(request_id)
    })
}

fn engine_error(error: &EngineClientError, request_id: Uuid) -> ApiError {
    match error {
        EngineClientError::Timeout => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::EngineTimeout,
            "Engine did not respond in time",
        )
        .with_request_id(request_id),
        EngineClientError::Rejected { .. } => ApiError::new(
            StatusCode::CONFLICT,
            ErrorCode::Internal,
            "Engine rejected the admin command",
        )
        .with_request_id(request_id),
        EngineClientError::Io
        | EngineClientError::Serialization
        | EngineClientError::Unexpected => ApiError::internal().with_request_id(request_id),
    }
}

fn forbidden(request_id: Uuid) -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        ErrorCode::Forbidden,
        "Admin privileges required",
    )
    .with_request_id(request_id)
}

fn not_found(message: &'static str, request_id: Uuid) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, message).with_request_id(request_id)
}
