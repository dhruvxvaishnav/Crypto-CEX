use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

static EMAIL_RE: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$")
});

/// Signup request body.
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SignupInput {
    /// Email address.
    #[validate(custom(function = "validate_email"))]
    pub email: String,
    /// Plaintext password.
    #[validate(custom(function = "validate_password"))]
    pub password: String,
}

/// Login request body.
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LoginInput {
    /// Email address.
    #[validate(custom(function = "validate_email"))]
    pub email: String,
    /// Plaintext password.
    pub password: String,
}

/// Refresh request body.
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RefreshInput {
    /// Opaque refresh token.
    #[validate(length(min = 32))]
    pub refresh_token: String,
}

/// Auth success response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokensResponse {
    /// JWT access token.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// Token type.
    pub token_type: &'static str,
    /// Access token TTL in seconds.
    pub expires_in: u64,
}

/// MFA-required login response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MfaRequiredResponse {
    /// Whether the client must complete MFA.
    pub mfa_required: bool,
    /// Short-lived MFA challenge token.
    pub mfa_token: String,
}

/// Health response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Health status.
    pub status: &'static str,
}

/// Ready response.
#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    /// Readiness status.
    pub status: &'static str,
}

/// Normalises email before storage and lookup.
#[must_use]
pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

/// Returns true when an email matches the accepted auth policy.
#[must_use]
pub fn is_valid_email(email: &str) -> bool {
    let normalised = normalize_email(email);
    normalised.len() <= 254
        && EMAIL_RE.as_ref().is_ok_and(|re| re.is_match(&normalised))
        && !normalised.contains('"')
}

fn validate_email(email: &str) -> Result<(), ValidationError> {
    if is_valid_email(email) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_email"))
    }
}

fn validate_password(password: &str) -> Result<(), ValidationError> {
    password_policy_failures(password)
        .is_empty()
        .then_some(())
        .ok_or_else(|| ValidationError::new("weak_password"))
}

/// Returns missing password policy requirements.
#[must_use]
pub fn password_policy_failures(password: &str) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if password.chars().count() < 12 {
        missing.push("min_length");
    }
    if !password.chars().any(char::is_lowercase) {
        missing.push("lowercase");
    }
    if !password.chars().any(char::is_uppercase) {
        missing.push("uppercase");
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        missing.push("digit");
    }
    if !password.chars().any(|c| !c.is_alphanumeric()) {
        missing.push("symbol");
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::{normalize_email, password_policy_failures, SignupInput};
    use validator::Validate;

    #[test]
    fn normalises_email_lowercase() {
        assert_eq!(normalize_email(" USER@Example.COM "), "user@example.com");
    }

    #[test]
    fn rejects_weak_password() {
        let input = SignupInput {
            email: "user@example.com".to_owned(),
            password: "weak".to_owned(),
        };

        assert!(input.validate().is_err());
        assert_eq!(
            password_policy_failures("weak"),
            vec!["min_length", "uppercase", "digit", "symbol"]
        );
    }
}
