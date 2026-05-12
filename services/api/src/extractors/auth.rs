use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::tokens::AccessClaims;
use crate::errors::{ApiError, ErrorCode};
use crate::state::AppState;

/// JWT-validated caller extracted from `Authorization: Bearer <token>`.
///
/// Pull this from handler arguments to gate a route behind authentication.
/// Verification is pure in-memory HMAC (~1–2 μs); no DB round-trip.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// User ID from the `sub` claim.
    pub user_id: Uuid,
    /// KYC level from the `kyc` claim.
    pub kyc_level: i32,
    /// Session ID from the `sid` claim.
    pub session_id: Uuid,
}

impl AuthenticatedUser {
    fn from_claims(claims: AccessClaims) -> Self {
        Self {
            user_id: claims.sub,
            kyc_level: claims.kyc,
            session_id: claims.sid,
        }
    }
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw = bearer_token(parts).ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                ErrorCode::TokenInvalid,
                "Missing or malformed Authorization header",
            )
        })?;

        let claims = state
            .token_config
            .decode_access_token(raw)
            .map_err(|err| {
                let is_expired = err
                    .to_string()
                    .to_ascii_lowercase()
                    .contains("expired");
                if is_expired {
                    ApiError::new(
                        StatusCode::UNAUTHORIZED,
                        ErrorCode::TokenExpired,
                        "Access token has expired",
                    )
                } else {
                    ApiError::new(
                        StatusCode::UNAUTHORIZED,
                        ErrorCode::TokenInvalid,
                        "Access token is invalid",
                    )
                }
            })?;

        Ok(Self::from_claims(claims))
    }
}

fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}
