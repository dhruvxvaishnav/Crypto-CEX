//! HMAC-SHA256 signature verification middleware (PRD §18.4, FR-API-02).
//!
//! The middleware reads the body once, verifies the signature, then
//! re-injects the body so handlers can read it normally.
//!
//! Signed payload: `{ts}\n{METHOD}\n{path}\n{rawBody}`.

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::errors::{ApiError, ErrorCode};
use crate::middleware::RequestContext;
use crate::state::AppState;

const HMAC_KEY_HEADER: &str = "x-aether-key";
const HMAC_TS_HEADER: &str = "x-aether-ts";
const HMAC_SIGN_HEADER: &str = "x-aether-sign";
/// Maximum allowed clock skew per PRD §18.4.
const MAX_SKEW_MS: i64 = 5_000;
/// Redis TTL for replay deduplication.
const REPLAY_TTL_SECS: u64 = 10;
/// Body size cap: 4 MiB.
const BODY_LIMIT: usize = 4 * 1024 * 1024;

/// Identity injected by successful HMAC verification.
#[derive(Debug, Clone)]
pub struct HmacCaller {
    /// Owning user ID from the API key record.
    pub user_id: Uuid,
    /// API key row ID.
    pub key_id: Uuid,
    /// Permissions granted to this key.
    pub permissions: Vec<String>,
}

/// Axum middleware that verifies HMAC-signed requests.
///
/// If `X-AETHER-KEY` is absent the request passes through unchanged.
/// On successful verification, a [`HmacCaller`] extension is inserted so that
/// downstream extractors can read it.
///
/// # Errors
///
/// Returns 401 with a stable code when any verification step fails.
pub async fn hmac_auth_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    // Only activate when the HMAC key header is present.
    if !request.headers().contains_key(HMAC_KEY_HEADER) {
        return next.run(request).await;
    }

    let ctx = request
        .extensions()
        .get::<RequestContext>()
        .copied()
        .unwrap_or(RequestContext { request_id: Uuid::new_v4() });

    match verify(request, &state, ctx).await {
        Ok(request) => next.run(request).await,
        Err(err) => err.into_response(),
    }
}

async fn verify(
    request: Request,
    state: &AppState,
    ctx: RequestContext,
) -> Result<Request, ApiError> {
    let key_id_str = header_str(&request, HMAC_KEY_HEADER)
        .ok_or_else(|| sig_error(ErrorCode::SigInvalid, "Missing X-AETHER-KEY", ctx))?;
    let ts_str = header_str(&request, HMAC_TS_HEADER)
        .ok_or_else(|| sig_error(ErrorCode::SigInvalid, "Missing X-AETHER-TS", ctx))?;
    let sign_str = header_str(&request, HMAC_SIGN_HEADER)
        .ok_or_else(|| sig_error(ErrorCode::SigInvalid, "Missing X-AETHER-SIGN", ctx))?;

    let key_id: Uuid = key_id_str
        .parse()
        .map_err(|_| sig_error(ErrorCode::SigInvalid, "Invalid key ID", ctx))?;
    let ts_ms: i64 = ts_str
        .parse()
        .map_err(|_| sig_error(ErrorCode::SigInvalid, "Invalid timestamp", ctx))?;

    // ── Timestamp window check (PRD §18.4) ───────────────────────────────────
    let now_ms = now_ms();
    let skew = (now_ms - ts_ms).abs();
    if skew > MAX_SKEW_MS {
        return Err(sig_error(ErrorCode::SigTimestamp, "Timestamp out of window", ctx));
    }

    // ── Load API key from DB ──────────────────────────────────────────────────
    let api_key = crate::repositories::account::find_api_key(&state.db, key_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .ok_or_else(|| sig_error(ErrorCode::SigInvalid, "Unknown API key", ctx))?;

    // ── Decrypt secret using pgcrypto ─────────────────────────────────────────
    let raw_secret =
        crate::repositories::account::decrypt_api_key_secret(&state.db, key_id, &state.pgcrypto_key)
            .await
            .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
            .ok_or_else(|| sig_error(ErrorCode::SigInvalid, "API key has no secret", ctx))?;

    // ── Buffer body for signature ─────────────────────────────────────────────
    let (mut parts, body) = request.into_parts();
    let body_bytes = axum::body::to_bytes(body, BODY_LIMIT)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

    // ── Build signed payload ──────────────────────────────────────────────────
    let method = parts.method.as_str();
    let path = parts.uri.path_and_query().map_or("/", |pq| pq.as_str());
    let body_str = std::str::from_utf8(&body_bytes).unwrap_or("");
    let payload = format!("{ts_ms}\n{method}\n{path}\n{body_str}");

    // ── Compute and verify HMAC-SHA256 ────────────────────────────────────────
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(raw_secret.as_bytes())
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;
    mac.update(payload.as_bytes());

    let expected_bytes = decode_hex(&sign_str)
        .ok_or_else(|| sig_error(ErrorCode::SigInvalid, "Invalid signature encoding", ctx))?;
    // Constant-time comparison via hmac::Mac::verify_slice.
    mac.verify_slice(&expected_bytes)
        .map_err(|_| sig_error(ErrorCode::SigInvalid, "Signature mismatch", ctx))?;

    // ── Replay check (PRD §18.4) ──────────────────────────────────────────────
    let sig_prefix: String = sign_str.chars().take(12).collect();
    let replay_key = format!("hmac_replay:{key_id}:{sig_prefix}");
    if let Err(err) = check_replay(&state.redis, &replay_key).await {
        return Err(err.with_request_id(ctx.request_id));
    }

    // ── Inject caller into extensions ─────────────────────────────────────────
    let permissions: Vec<String> = api_key
        .permissions
        .iter()
        .map(|p| p.to_string())
        .collect();
    parts.extensions.insert(HmacCaller {
        user_id: api_key.user_id,
        key_id,
        permissions,
    });

    let request = Request::from_parts(parts, Body::from(body_bytes));
    Ok(request)
}

async fn check_replay(
    redis: &Option<redis::aio::ConnectionManager>,
    key: &str,
) -> Result<(), ApiError> {
    let Some(conn) = redis else {
        return Ok(());
    };
    let mut conn = conn.clone();
    // SET NX EX: if key already exists → replay.
    let set: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg("1")
        .arg("NX")
        .arg("EX")
        .arg(REPLAY_TTL_SECS)
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    if set.is_none() {
        // Key already existed → replay attack.
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::SigReplay,
            "Signature replay detected",
        ));
    }
    Ok(())
}

fn header_str(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned)
}

fn sig_error(code: ErrorCode, message: &'static str, ctx: RequestContext) -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, code, message).with_request_id(ctx.request_id)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}
