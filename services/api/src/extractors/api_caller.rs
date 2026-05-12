//! `ApiCaller` extractor: accepts either a valid JWT or a verified HMAC key.
//!
//! JWT auth is attempted first. If no `Authorization: Bearer` header is
//! present, the extractor checks for a [`HmacCaller`] extension inserted by
//! the HMAC middleware. Either route produces an [`ApiCaller`] with a valid
//! `user_id`.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::errors::{ApiError, ErrorCode};
use crate::extractors::auth::AuthenticatedUser;
use crate::extractors::hmac::HmacCaller;
use crate::state::AppState;

/// Unified caller identity for routes that accept both JWT and HMAC auth.
#[derive(Debug, Clone)]
pub struct ApiCaller {
    /// User ID, identical regardless of auth method.
    pub user_id: Uuid,
    /// KYC level (from JWT) or 0 (HMAC).
    pub kyc_level: i32,
    /// Session ID (JWT) or API key ID (HMAC).
    pub session_id: Uuid,
    /// Whether this call was authenticated via HMAC.
    pub is_hmac: bool,
    /// Permissions (only meaningful for HMAC callers).
    pub hmac_permissions: Vec<String>,
}

impl FromRequestParts<AppState> for ApiCaller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // If an Authorization: Bearer header is present, enforce JWT auth.
        if has_bearer(parts) {
            let user = AuthenticatedUser::from_request_parts(parts, state).await?;
            return Ok(Self {
                user_id: user.user_id,
                kyc_level: user.kyc_level,
                session_id: user.session_id,
                is_hmac: false,
                hmac_permissions: vec![],
            });
        }

        // Fall back to HMAC caller injected by the middleware.
        if let Some(hmac) = parts.extensions.get::<HmacCaller>().cloned() {
            return Ok(Self {
                user_id: hmac.user_id,
                kyc_level: 0,
                session_id: hmac.key_id,
                is_hmac: true,
                hmac_permissions: hmac.permissions,
            });
        }

        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::TokenInvalid,
            "Authentication required (Bearer token or HMAC headers)",
        ))
    }
}

fn has_bearer(parts: &Parts) -> bool {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer "))
}
