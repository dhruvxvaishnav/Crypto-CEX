//! TOTP 2FA endpoints (PRD §7.1 FR-AUTH-03).
//!
//! Flow:
//!  1. `POST /auth/2fa/setup`  — generates secret + QR, stores encrypted
//!  2. `POST /auth/2fa/verify` — verifies code, enables 2FA, returns backup codes
//!  3. `POST /auth/2fa/disable`— disables 2FA (requires live TOTP code + password)

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};

use crate::auth::password::verify_password;
use crate::errors::{ApiError, ErrorCode};
use crate::extractors::auth::AuthenticatedUser;
use crate::middleware::RequestContext;
use crate::repositories::account as repo;
use crate::state::AppState;

const TOTP_ISSUER: &str = "Aether Exchange";
const BACKUP_CODE_COUNT: usize = 8;
const BACKUP_CODE_BYTES: usize = 6;
/// 20 bytes = 160-bit TOTP secret (SHA-1 standard).
const TOTP_SECRET_BYTES: usize = 20;

// ── Input types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyTotpInput {
    /// 6-digit TOTP code from the authenticator app.
    pub code: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisableTotpInput {
    /// Current TOTP code for confirmation.
    pub code: String,
    /// Current password for confirmation.
    pub password: String,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupTotpResponse {
    /// Base32-encoded TOTP secret (for manual entry).
    secret: String,
    /// `otpauth://` URI for QR scan.
    otp_auth_uri: String,
    /// Base64-encoded PNG data URI of the QR code.
    qr: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyTotpResponse {
    /// Whether 2FA is now enabled.
    enabled: bool,
    /// One-time backup codes — store them securely; not shown again.
    backup_codes: Vec<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `POST /auth/2fa/setup` — Generate secret and QR code.
///
/// Idempotent: calling again regenerates and overwrites the stored (unverified) secret.
///
/// # Errors
///
/// Returns [`ApiError`] if the user profile is unavailable or DB fails.
pub async fn setup_totp(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let profile = repo::get_profile(&state.db, caller.user_id)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "totp.setup.db_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, "User not found")
                .with_request_id(ctx.request_id)
        })?;

    if profile.totp_enabled {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            ErrorCode::Forbidden,
            "2FA is already enabled; disable it first",
        )
        .with_request_id(ctx.request_id));
    }

    // Generate a 20-byte random secret and encode as base32.
    let mut key_bytes = [0_u8; TOTP_SECRET_BYTES];
    OsRng.fill_bytes(&mut key_bytes);
    // Secret::Raw(bytes).to_string() gives the base32 representation.
    let raw_secret = Secret::Raw(key_bytes.to_vec());
    let secret_base32 = raw_secret.to_string();

    let totp = build_totp(key_bytes.to_vec(), profile.email, ctx.request_id)?;

    let otp_auth_uri = totp.get_url();
    let qr_base64 = totp.get_qr_base64().map_err(|err| {
        tracing::error!(err = %err, "totp.setup.qr_error");
        ApiError::internal().with_request_id(ctx.request_id)
    })?;
    let qr_data_uri = format!("data:image/png;base64,{qr_base64}");

    // Persist encrypted secret.
    repo::store_totp_secret(&state.db, caller.user_id, &secret_base32, &state.pgcrypto_key)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "totp.setup.store_error");
            ApiError::internal().with_request_id(ctx.request_id)
        })?;

    tracing::info!(event = "auth.totp.setup", user_id = %caller.user_id);

    Ok(Json(SetupTotpResponse {
        secret: secret_base32,
        otp_auth_uri,
        qr: qr_data_uri,
    }))
}

/// `POST /auth/2fa/verify` — Verify code and enable 2FA.
///
/// # Errors
///
/// Returns [`ApiError`] when the code is invalid, 2FA is already on, or DB fails.
pub async fn verify_totp(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
    payload: Result<Json<VerifyTotpInput>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| {
        ApiError::new(StatusCode::BAD_REQUEST, ErrorCode::Internal, "Invalid JSON")
            .with_request_id(ctx.request_id)
    })?;

    let profile = repo::get_profile(&state.db, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, "User not found")
                .with_request_id(ctx.request_id)
        })?;

    if profile.totp_enabled {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            ErrorCode::Forbidden,
            "2FA is already enabled",
        )
        .with_request_id(ctx.request_id));
    }

    let totp = load_totp(&state, caller.user_id, profile.email, ctx.request_id).await?;

    let valid = totp.check_current(&input.code).map_err(|err| {
        tracing::error!(err = %err, "totp.verify.check_error");
        ApiError::internal().with_request_id(ctx.request_id)
    })?;

    if !valid {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::MfaInvalid,
            "Invalid TOTP code",
        )
        .with_request_id(ctx.request_id));
    }

    repo::enable_totp(&state.db, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

    let backup_codes = generate_backup_codes();
    tracing::info!(event = "auth.totp.enabled", user_id = %caller.user_id);

    Ok(Json(VerifyTotpResponse {
        enabled: true,
        backup_codes,
    }))
}

/// `POST /auth/2fa/disable` — Disable 2FA (requires live TOTP code + password).
///
/// # Errors
///
/// Returns [`ApiError`] when credentials are invalid or DB fails.
pub async fn disable_totp(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
    payload: Result<Json<DisableTotpInput>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| {
        ApiError::new(StatusCode::BAD_REQUEST, ErrorCode::Internal, "Invalid JSON")
            .with_request_id(ctx.request_id)
    })?;

    let user = repo::get_user_for_auth(&state.db, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, "User not found")
                .with_request_id(ctx.request_id)
        })?;

    let password_ok = verify_password(&input.password, &user.password_hash)
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

    if !password_ok {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidCredentials,
            "Invalid password",
        )
        .with_request_id(ctx.request_id));
    }

    if !user.totp_enabled {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            ErrorCode::Forbidden,
            "2FA is not enabled",
        )
        .with_request_id(ctx.request_id));
    }

    let totp = load_totp(&state, caller.user_id, user.email, ctx.request_id).await?;

    let valid = totp
        .check_current(&input.code)
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

    if !valid {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::MfaInvalid,
            "Invalid TOTP code",
        )
        .with_request_id(ctx.request_id));
    }

    repo::disable_totp(&state.db, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

    tracing::info!(event = "auth.totp.disabled", user_id = %caller.user_id);

    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Retrieves the encrypted TOTP secret from DB and builds a [`TOTP`] instance.
async fn load_totp(
    state: &AppState,
    user_id: uuid::Uuid,
    email: String,
    request_id: uuid::Uuid,
) -> Result<TOTP, ApiError> {
    let secret_base32 =
        repo::decrypt_totp_secret(&state.db, user_id, &state.pgcrypto_key)
            .await
            .map_err(|_| ApiError::internal().with_request_id(request_id))?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    ErrorCode::MfaInvalid,
                    "TOTP setup not initiated; call /auth/2fa/setup first",
                )
                .with_request_id(request_id)
            })?;

    let secret_bytes = Secret::Encoded(secret_base32)
        .to_bytes()
        .map_err(|_| ApiError::internal().with_request_id(request_id))?;

    build_totp(secret_bytes, email, request_id)
}

fn build_totp(
    secret_bytes: Vec<u8>,
    account_name: String,
    request_id: uuid::Uuid,
) -> Result<TOTP, ApiError> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some(TOTP_ISSUER.to_owned()),
        account_name,
    )
    .map_err(|err| {
        tracing::error!(err = %err, "totp.build_error");
        ApiError::internal().with_request_id(request_id)
    })
}

/// Generates 8 random one-time backup codes as URL-safe base64 strings.
fn generate_backup_codes() -> Vec<String> {
    (0..BACKUP_CODE_COUNT)
        .map(|_| {
            let mut bytes = [0_u8; BACKUP_CODE_BYTES];
            OsRng.fill_bytes(&mut bytes);
            URL_SAFE_NO_PAD.encode(bytes)
        })
        .collect()
}
