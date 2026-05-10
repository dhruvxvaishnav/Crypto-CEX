use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Token minting configuration.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// HS256 secret.
    pub jwt_secret: String,
    /// Access-token TTL.
    pub access_token_ttl: Duration,
    /// MFA-token TTL.
    pub mfa_token_ttl: Duration,
    /// Refresh-token TTL.
    pub refresh_token_ttl: Duration,
}

/// Access token claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Subject user ID.
    pub sub: Uuid,
    /// Session ID.
    pub sid: Uuid,
    /// KYC level.
    pub kyc: i32,
    /// Issued-at timestamp.
    pub iat: i64,
    /// Expiry timestamp.
    pub exp: i64,
    /// JWT ID.
    pub jti: Uuid,
}

/// MFA token claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaClaims {
    /// Subject user ID.
    pub sub: Uuid,
    /// Session ID.
    pub sid: Uuid,
    /// Token purpose.
    pub purpose: &'static str,
    /// Issued-at timestamp.
    pub iat: i64,
    /// Expiry timestamp.
    pub exp: i64,
    /// JWT ID.
    pub jti: Uuid,
}

/// Minted access and refresh token pair.
#[derive(Debug, Clone)]
pub struct TokenPair {
    /// JWT access token.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
    /// Hash stored in Postgres.
    pub refresh_token_hash: String,
    /// Refresh token expiry.
    pub refresh_expires_at: OffsetDateTime,
    /// Access token TTL in seconds.
    pub expires_in: u64,
}

/// Token minting error.
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    /// JWT encode failed.
    #[error("jwt encode failed")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    /// Token expiry overflowed.
    #[error("token expiry overflow")]
    ExpiryOverflow,
}

impl TokenConfig {
    /// Mints an access token and a refresh token.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when JWT encoding or expiry math fails.
    pub fn mint_pair(
        &self,
        now: OffsetDateTime,
        user_id: Uuid,
        session_id: Uuid,
        kyc_level: i32,
        jwt_id: Uuid,
    ) -> Result<TokenPair, TokenError> {
        let access_expires_at =
            add_duration(now, self.access_token_ttl).ok_or(TokenError::ExpiryOverflow)?;
        let refresh_expires_at =
            add_duration(now, self.refresh_token_ttl).ok_or(TokenError::ExpiryOverflow)?;
        let claims = AccessClaims {
            sub: user_id,
            sid: session_id,
            kyc: kyc_level,
            iat: now.unix_timestamp(),
            exp: access_expires_at.unix_timestamp(),
            jti: jwt_id,
        };
        let access_token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        let refresh_token = generate_refresh_token();
        let refresh_token_hash = hash_refresh_token(&refresh_token);
        Ok(TokenPair {
            access_token,
            refresh_token,
            refresh_token_hash,
            refresh_expires_at,
            expires_in: self.access_token_ttl.as_secs(),
        })
    }

    /// Mints a short-lived MFA challenge token.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when JWT encoding or expiry math fails.
    pub fn mint_mfa_token(
        &self,
        now: OffsetDateTime,
        user_id: Uuid,
        session_id: Uuid,
        jwt_id: Uuid,
    ) -> Result<String, TokenError> {
        let expires_at = add_duration(now, self.mfa_token_ttl).ok_or(TokenError::ExpiryOverflow)?;
        let claims = MfaClaims {
            sub: user_id,
            sid: session_id,
            purpose: "mfa",
            iat: now.unix_timestamp(),
            exp: expires_at.unix_timestamp(),
            jti: jwt_id,
        };
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(TokenError::from)
    }
}

/// Returns SHA-256 hex digest for a refresh token.
#[must_use]
pub fn hash_refresh_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(hex_char(byte >> 4));
        out.push(hex_char(byte & 0x0f));
    }
    out
}

fn hex_char(nibble: u8) -> char {
    HEX.get(usize::from(nibble))
        .copied()
        .map_or('0', char::from)
}

fn generate_refresh_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn add_duration(now: OffsetDateTime, duration: Duration) -> Option<OffsetDateTime> {
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    now.checked_add(time::Duration::seconds(seconds))
}
