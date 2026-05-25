use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{ApiError, ErrorCode};
use crate::extractors::auth::AuthenticatedUser;
use crate::middleware::RequestContext;
use crate::repositories::account as repo;
use crate::state::AppState;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileResponse {
    id: Uuid,
    email: String,
    status: String,
    kyc_level: i32,
    email_verified: bool,
    totp_enabled: bool,
    is_admin: bool,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BalanceResponse {
    asset: String,
    asset_name: String,
    available: String,
    locked: String,
    total: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerEntryResponse {
    id: i64,
    asset: String,
    kind: String,
    amount: String,
    available_after: String,
    locked_after: String,
    reference_type: String,
    reference_id: Option<Uuid>,
    ts: String,
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    cursor: Option<i64>,
    #[serde(default = "default_limit")]
    limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct FaucetInput {
    pub asset: String,
    pub amount: String,
}

const fn default_limit() -> i64 {
    50
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /account`
///
/// # Errors
///
/// Returns [`ApiError`] when the user is not found or DB fails.
pub async fn get_profile(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let row = repo::get_profile(&state.db, caller.user_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "account.profile.db_error");
            ApiError::internal()
        })?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, "User not found")
        })?;

    Ok(Json(ProfileResponse {
        id: row.id,
        email: row.email,
        status: row.status,
        kyc_level: row.kyc_level,
        email_verified: row.email_verified,
        totp_enabled: row.totp_enabled,
        is_admin: row.is_admin,
        created_at: row.created_at.to_string(),
    }))
}

/// `GET /account/balances`
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure.
pub async fn get_balances(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let rows = repo::list_balances(&state.db, caller.user_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "account.balances.db_error");
            ApiError::internal()
        })?;

    let body: Vec<BalanceResponse> = rows
        .into_iter()
        .map(|r| {
            let total = r.available + r.locked;
            BalanceResponse {
                asset: r.asset_symbol,
                asset_name: r.asset_name,
                available: r.available.to_string(),
                locked: r.locked.to_string(),
                total: total.to_string(),
            }
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": body })))
}

/// `GET /account/history?cursor=<id>&limit=50`
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure.
pub async fn get_history(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Query(q): Query<HistoryQuery>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.clamp(1, 200);
    let rows = repo::ledger_history(&state.db, caller.user_id, q.cursor, limit)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "account.history.db_error");
            ApiError::internal()
        })?;

    let next_cursor = rows.last().map(|r| r.id);
    let body: Vec<LedgerEntryResponse> = rows
        .into_iter()
        .map(|r| LedgerEntryResponse {
            id: r.id,
            asset: r.asset_symbol,
            kind: r.kind,
            amount: r.amount.to_string(),
            available_after: r.available_after.to_string(),
            locked_after: r.locked_after.to_string(),
            reference_type: r.reference_type,
            reference_id: r.reference_id,
            ts: r.created_at.to_string(),
        })
        .collect();

    Ok(Json(serde_json::json!({
        "data": body,
        "nextCursor": next_cursor
    })))
}

/// `POST /wallet/faucet`
///
/// # Errors
///
/// Returns [`ApiError`] on bad asset, exceeding faucet limit, or DB failure.
pub async fn faucet(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
    payload: Result<Json<FaucetInput>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "Invalid JSON body",
        )
        .with_request_id(ctx.request_id)
    })?;

    let amount = input.amount.parse::<rust_decimal::Decimal>().map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "Invalid amount format",
        )
        .with_request_id(ctx.request_id)
    })?;

    if amount <= rust_decimal::Decimal::ZERO {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "Amount must be positive",
        )
        .with_request_id(ctx.request_id));
    }

    let (asset_id, faucet_max) = repo::get_faucet_limit(&state.db, &input.asset.to_uppercase())
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "wallet.faucet.db_error");
            ApiError::internal()
        })?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                ErrorCode::NotFound,
                "Asset not found",
            )
            .with_request_id(ctx.request_id)
        })?;

    if amount > faucet_max {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::InsufficientBalance,
            format!("Faucet limit is {faucet_max}"),
        )
        .with_request_id(ctx.request_id));
    }

    let deposit_id = Uuid::new_v4();
    let new_available = repo::apply_faucet(&state.db, caller.user_id, asset_id, amount, deposit_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "wallet.faucet.apply_error");
            ApiError::internal()
        })?;

    tracing::info!(
        event = "wallet.faucet.credited",
        user_id = %caller.user_id,
        asset = %input.asset,
        amount = %amount,
        deposit_id = %deposit_id
    );

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "depositId": deposit_id,
            "asset": input.asset.to_uppercase(),
            "amount": amount.to_string(),
            "availableAfter": new_available.to_string()
        })),
    ))
}
