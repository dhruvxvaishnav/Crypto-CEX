use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use validator::Validate;

use crate::auth::service::AuthError;
use crate::auth::LoginOutcome;
use crate::errors::{ApiError, ErrorCode};
use crate::middleware::RequestContext;
use crate::schemas::{
    password_policy_failures, AuthTokensResponse, LoginInput, MfaRequiredResponse, RefreshInput,
    SignupInput,
};
use crate::state::AppState;

/// Signup endpoint.
///
/// # Errors
///
/// Returns [`ApiError`] when the body is invalid, the email is taken, or auth
/// persistence/token minting fails.
pub async fn signup(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    payload: Result<Json<SignupInput>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| json_error(context.request_id))?;
    validate_signup(&input, context.request_id)?;
    let token_pair = state
        .auth
        .signup(input)
        .await
        .map_err(|error| map_auth_error(error, context.request_id))?;
    Ok((
        StatusCode::CREATED,
        Json(AuthTokensResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: "Bearer",
            expires_in: token_pair.expires_in,
        }),
    ))
}

/// Login endpoint.
///
/// # Errors
///
/// Returns [`ApiError`] when credentials are invalid, attempts are rate-limited,
/// MFA/token minting fails, or auth persistence is unavailable.
pub async fn login(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    payload: Result<Json<LoginInput>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| json_error(context.request_id))?;
    input
        .validate()
        .map_err(|_| invalid_email(context.request_id))?;
    let ip_key = client_ip_key(&headers);
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    match state
        .auth
        .login(input, ip_key, user_agent)
        .await
        .map_err(|error| map_auth_error(error, context.request_id))?
    {
        LoginOutcome::Tokens(token_pair) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "accessToken": token_pair.access_token,
                "refreshToken": token_pair.refresh_token,
                "tokenType": "Bearer",
                "expiresIn": token_pair.expires_in
            })),
        )),
        LoginOutcome::MfaRequired { mfa_token } => Ok((
            StatusCode::OK,
            Json(
                serde_json::to_value(MfaRequiredResponse {
                    mfa_required: true,
                    mfa_token,
                })
                .map_err(|_| ApiError::internal().with_request_id(context.request_id))?,
            ),
        )),
    }
}

/// Refresh endpoint.
///
/// # Errors
///
/// Returns [`ApiError`] when the refresh token is invalid, expired, reused, or
/// auth persistence/token minting fails.
pub async fn refresh(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    payload: Result<Json<RefreshInput>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| json_error(context.request_id))?;
    input
        .validate()
        .map_err(|_| token_invalid(context.request_id))?;
    let token_pair = state
        .auth
        .refresh(&input.refresh_token)
        .await
        .map_err(|error| map_auth_error(error, context.request_id))?;
    Ok((
        StatusCode::OK,
        Json(AuthTokensResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: "Bearer",
            expires_in: token_pair.expires_in,
        }),
    ))
}

fn validate_signup(input: &SignupInput, request_id: uuid::Uuid) -> Result<(), ApiError> {
    if input.validate().is_ok() {
        return Ok(());
    }
    if !password_policy_failures(&input.password).is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::WeakPassword,
            "Password does not meet policy",
        )
        .with_details(serde_json::json!({
            "missing": password_policy_failures(&input.password)
        }))
        .with_request_id(request_id));
    }
    Err(invalid_email(request_id))
}

fn map_auth_error(error: AuthError, request_id: uuid::Uuid) -> ApiError {
    match error {
        AuthError::InvalidEmail => invalid_email(request_id),
        AuthError::WeakPassword { missing } => ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::WeakPassword,
            "Password does not meet policy",
        )
        .with_details(serde_json::json!({ "missing": missing }))
        .with_request_id(request_id),
        AuthError::EmailTaken => ApiError::new(
            StatusCode::CONFLICT,
            ErrorCode::EmailTaken,
            "Email is already registered",
        )
        .with_request_id(request_id),
        AuthError::InvalidCredentials => ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidCredentials,
            "Invalid credentials",
        )
        .with_request_id(request_id),
        AuthError::TooManyAttempts => ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            "Too many login attempts",
        )
        .with_request_id(request_id),
        AuthError::TokenExpired => ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::TokenExpired,
            "Refresh token expired",
        )
        .with_request_id(request_id),
        AuthError::TokenInvalid => token_invalid(request_id),
        AuthError::Internal => ApiError::internal().with_request_id(request_id),
    }
}

fn json_error(request_id: uuid::Uuid) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        ErrorCode::Internal,
        "Invalid JSON body",
    )
    .with_request_id(request_id)
}

fn invalid_email(request_id: uuid::Uuid) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidEmail,
        "Invalid email",
    )
    .with_request_id(request_id)
}

fn token_invalid(request_id: uuid::Uuid) -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        ErrorCode::TokenInvalid,
        "Refresh token is invalid",
    )
    .with_request_id(request_id)
}

fn client_ip_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_owned()
}
