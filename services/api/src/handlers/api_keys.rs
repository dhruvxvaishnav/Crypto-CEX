//! API key CRUD endpoints (PRD §7.5 FR-API-01).
//!
//! The secret is returned **once** on creation; after that only the key ID is
//! visible. The secret is stored with `pgp_sym_encrypt` so it can be
//! decrypted for HMAC verification. See `docs/clarifications.md` for why
//! `argon2id` (PRD draft) cannot be used with HMAC.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{ApiError, ErrorCode};
use crate::extractors::auth::AuthenticatedUser;
use crate::middleware::RequestContext;
use crate::repositories::account as repo;
use crate::state::AppState;

const MAX_LABEL_LEN: usize = 64;

/// Allowed permission values.
const ALLOWED_PERMISSIONS: &[&str] = &["read", "trade"];

// ── Input / output types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyInput {
    /// Human-readable label (max 64 chars).
    pub label: String,
    /// Subset of `["read", "trade"]`. Withdraw is disabled in v1.
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyCreatedResponse {
    key_id: Uuid,
    label: String,
    permissions: Vec<String>,
    /// Raw secret — shown **once**; client must store it.
    secret: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyListItem {
    key_id: Uuid,
    label: String,
    permissions: Vec<String>,
    created_at: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `POST /account/api-keys` — Create an API key.
///
/// # Errors
///
/// Returns [`ApiError`] on validation failure or DB error.
pub async fn create_api_key(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
    payload: Result<Json<CreateApiKeyInput>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "Invalid JSON body",
        )
        .with_request_id(ctx.request_id)
    })?;

    // Validate label.
    if input.label.trim().is_empty() || input.label.len() > MAX_LABEL_LEN {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "label must be 1–64 characters",
        )
        .with_request_id(ctx.request_id));
    }

    // Validate permissions.
    for perm in &input.permissions {
        if perm == "withdraw" {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Forbidden,
                "withdraw permission is disabled in v1",
            )
            .with_request_id(ctx.request_id));
        }
        if !ALLOWED_PERMISSIONS.contains(&perm.as_str()) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Internal,
                format!("unknown permission: {perm}"),
            )
            .with_request_id(ctx.request_id));
        }
    }

    if input.permissions.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "at least one permission is required",
        )
        .with_request_id(ctx.request_id));
    }

    // Generate a 32-byte cryptographically random secret.
    let mut secret_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut secret_bytes);
    let raw_secret = URL_SAFE_NO_PAD.encode(secret_bytes);

    let key = repo::create_api_key(
        &state.db,
        caller.user_id,
        input.label.trim(),
        &raw_secret,
        &input.permissions,
        &state.pgcrypto_key,
    )
    .await
    .map_err(|err| {
        tracing::error!(
            err = %err,
            user_id = %caller.user_id,
            request_id = %ctx.request_id,
            "api_keys.create.db_error"
        );
        ApiError::internal().with_request_id(ctx.request_id)
    })?;

    tracing::info!(
        event = "api_key.created",
        user_id = %caller.user_id,
        key_id = %key.id,
        label = %key.label
    );

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyCreatedResponse {
            key_id: key.id,
            label: key.label,
            permissions: key.permissions,
            secret: raw_secret,
            created_at: key.created_at.to_string(),
        }),
    ))
}

/// `GET /account/api-keys` — List active API keys.
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure.
pub async fn list_api_keys(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let keys = repo::list_api_keys(&state.db, caller.user_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "api_keys.list.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;

    let body: Vec<ApiKeyListItem> = keys
        .into_iter()
        .map(|k| ApiKeyListItem {
            key_id: k.id,
            label: k.label,
            permissions: k.permissions,
            created_at: k.created_at.to_string(),
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": body })))
}

/// `DELETE /account/api-keys/:id` — Revoke an API key.
///
/// # Errors
///
/// Returns [`ApiError`] when the key is not found or DB fails.
pub async fn revoke_api_key(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(key_id): Path<Uuid>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let revoked = repo::revoke_api_key(&state.db, caller.user_id, key_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "api_keys.revoke.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;

    if !revoked {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            "API key not found",
        )
        .with_request_id(ctx.request_id));
    }

    tracing::info!(
        event = "api_key.revoked",
        user_id = %caller.user_id,
        key_id = %key_id
    );

    Ok(StatusCode::NO_CONTENT)
}
