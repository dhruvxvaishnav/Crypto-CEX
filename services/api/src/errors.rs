use std::collections::BTreeMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;
use validator::ValidationErrors;

/// Stable API error codes shared with `packages/shared/src/errors.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Invalid email address.
    InvalidEmail,
    /// Password did not satisfy policy.
    WeakPassword,
    /// Email is already registered.
    EmailTaken,
    /// Credentials are invalid.
    InvalidCredentials,
    /// MFA challenge is required.
    MfaRequired,
    /// Token expired.
    TokenExpired,
    /// Token is invalid.
    TokenInvalid,
    /// Resource or route not found.
    NotFound,
    /// Rate limit exceeded.
    RateLimited,
    /// Engine did not respond in time.
    EngineTimeout,
    /// Internal service error.
    Internal,
}

/// JSON error response envelope.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    /// Error payload.
    pub error: ErrorBody,
    /// Request correlation ID.
    pub request_id: Uuid,
}

/// JSON error body.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// Stable error code.
    pub code: ErrorCode,
    /// Safe user-facing message.
    pub message: String,
    /// Optional structured details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// API error that can be rendered as the standard envelope.
#[derive(Debug, Clone)]
pub struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
    details: Option<Value>,
    request_id: Option<Uuid>,
}

impl ApiError {
    /// Creates an API error.
    #[must_use]
    pub fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
            request_id: None,
        }
    }

    /// Attaches structured details.
    #[must_use]
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Attaches request ID to the response envelope.
    #[must_use]
    pub const fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Creates a generic internal error.
    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            "Something went wrong",
        )
    }

    /// Creates a validation error response.
    #[must_use]
    pub fn validation(errors: &ValidationErrors) -> Self {
        let mut details = BTreeMap::new();
        for (field, field_errors) in errors.field_errors() {
            let codes: Vec<String> = field_errors
                .iter()
                .map(|error| error.code.to_string())
                .collect();
            details.insert((*field).to_owned(), codes);
        }
        Self::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::WeakPassword,
            "Request validation failed",
        )
        .with_details(serde_json::json!({ "fields": details }))
    }

    /// Returns the HTTP status for logging and tests.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = self.request_id.unwrap_or_else(Uuid::nil);
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
            request_id,
        };
        (self.status, Json(body)).into_response()
    }
}
