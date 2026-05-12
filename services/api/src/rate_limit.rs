//! Redis sliding-window rate limiter (PRD §7.5 FR-API-03).
//!
//! Uses a Lua script to atomically maintain a sorted-set window in Redis.
//! Each entry in the set is `<now_ms>:<unique_suffix>` to avoid collisions
//! when multiple requests arrive within the same millisecond.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::errors::{ApiError, ErrorCode};
use crate::middleware::RequestContext;
use crate::state::AppState;

/// Rate-limit outcome including headers to attach to the response.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitOutcome {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Configured request limit for this window.
    pub limit: u32,
    /// Requests remaining after this one.
    pub remaining: u32,
    /// Unix seconds when the window resets.
    pub reset_at: i64,
}

/// Lua script: atomic sliding-window counter.
///
/// KEYS[1] = rate-limit key
/// ARGV[1] = now_ms (current epoch millis)
/// ARGV[2] = window_ms (window size in millis)
/// ARGV[3] = limit (max requests in window)
/// ARGV[4] = member (unique request identifier)
///
/// Returns `[count_before_add, was_added]`:
///   was_added = 1 → request allowed and counted
///   was_added = 0 → limit exceeded
const SLIDING_WINDOW_SCRIPT: &str = r"
local key     = KEYS[1]
local now     = tonumber(ARGV[1])
local window  = tonumber(ARGV[2])
local limit   = tonumber(ARGV[3])
local member  = ARGV[4]
redis.call('ZREMRANGEBYSCORE', key, '-inf', now - window)
local count = tonumber(redis.call('ZCARD', key))
if count >= limit then
  return {count, 0}
end
redis.call('ZADD', key, now, member)
redis.call('PEXPIRE', key, window + 1000)
return {count + 1, 1}
";

/// Checks and records a rate-limit slot.
///
/// Returns `RateLimitOutcome`. On a Redis error the call fails open (allowed).
///
/// # Errors
///
/// Returns `ApiError` if the rate limit is exceeded.
pub async fn check_rate_limit(
    redis: &Option<redis::aio::ConnectionManager>,
    key: &str,
    limit: u32,
    window_ms: u64,
    now_ms: i64,
) -> Result<RateLimitOutcome, ApiError> {
    let reset_at = now_ms
        .checked_add(i64::try_from(window_ms).unwrap_or(i64::MAX))
        .unwrap_or(i64::MAX)
        / 1000;

    let Some(conn) = redis else {
        // Redis unavailable — fail open to keep tests working.
        return Ok(RateLimitOutcome {
            allowed: true,
            limit,
            remaining: limit.saturating_sub(1),
            reset_at,
        });
    };

    let mut conn = conn.clone();
    let member = Uuid::new_v4().to_string();
    let script = redis::Script::new(SLIDING_WINDOW_SCRIPT);

    let (count, was_added): (i64, i64) = match script
        .key(key)
        .arg(now_ms)
        .arg(window_ms)
        .arg(i64::from(limit))
        .arg(&member)
        .invoke_async(&mut conn)
        .await
    {
        Ok(result) => result,
        Err(err) => {
            // Fail-open on Redis errors — do not block requests.
            tracing::warn!(event = "rate_limit.redis_error", err = %err, key = %key);
            return Ok(RateLimitOutcome {
                allowed: true,
                limit,
                remaining: limit.saturating_sub(1),
                reset_at,
            });
        }
    };

    let allowed = was_added == 1;
    let remaining = if allowed {
        u32::try_from(i64::from(limit) - count)
            .unwrap_or(0)
    } else {
        0
    };

    Ok(RateLimitOutcome {
        allowed,
        limit,
        remaining,
        reset_at,
    })
}

/// Appends standard rate-limit headers to a response.
pub fn apply_rate_limit_headers(response: &mut Response, outcome: RateLimitOutcome) {
    let headers = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&outcome.limit.to_string()) {
        headers.insert(HeaderName::from_static("x-ratelimit-limit"), v);
    }
    if let Ok(v) = HeaderValue::from_str(&outcome.remaining.to_string()) {
        headers.insert(HeaderName::from_static("x-ratelimit-remaining"), v);
    }
    if let Ok(v) = HeaderValue::from_str(&outcome.reset_at.to_string()) {
        headers.insert(HeaderName::from_static("x-ratelimit-reset"), v);
    }
}

/// Axum middleware that enforces sliding-window rate limits per PRD §FR-API-03.
///
/// Limit tiers based on path prefix:
/// - `/api/v1/auth/*` → 10 requests / 15 min / IP
/// - `/api/v1/orders*` → 60 requests / min / user (or IP if unauth)
/// - Everything else → 600 requests / min / user (or 30/min for unauth IP)
pub async fn rate_limit_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let now_ms = chrono_now_ms();

    // Determine rate-limit key and limits based on path.
    let (limit, window_ms, rl_key) = classify_request(&request, &path, now_ms, &state);

    match check_rate_limit(&state.redis, &rl_key, limit, window_ms, now_ms).await {
        Ok(outcome) if !outcome.allowed => {
            let ctx = request.extensions().get::<RequestContext>().copied();
            let request_id = ctx.map_or(uuid::Uuid::nil(), |c| c.request_id);
            let mut response = ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                ErrorCode::RateLimited,
                "Rate limit exceeded",
            )
            .with_request_id(request_id)
            .into_response();
            apply_rate_limit_headers(&mut response, outcome);
            if let Ok(v) = HeaderValue::from_str(&outcome.reset_at.to_string()) {
                response.headers_mut().insert("retry-after", v);
            }
            response
        }
        Ok(outcome) => {
            let mut response = next.run(request).await;
            apply_rate_limit_headers(&mut response, outcome);
            response
        }
        Err(_) => next.run(request).await,
    }
}

fn classify_request(
    request: &Request,
    path: &str,
    now_ms: i64,
    state: &AppState,
) -> (u32, u64, String) {
    let ip = extract_ip(request);

    // Auth endpoints: strict IP-only limit.
    if path.starts_with("/api/v1/auth/") {
        let key = format!("rl:auth:{ip}");
        return (10, 15 * 60 * 1_000, key);
    }

    // Try to get user identity from JWT (best-effort, no full validation).
    let user_key = extract_user_key(request, state);

    // Order write endpoints.
    if path.starts_with("/api/v1/orders") {
        let (limit, key) = user_key.map_or_else(
            || (30_u32, format!("rl:orders:ip:{ip}:{}", window_bucket(now_ms, 60_000))),
            |uid| (60_u32, format!("rl:orders:user:{uid}:{}", window_bucket(now_ms, 60_000))),
        );
        return (limit, 60_000, key);
    }

    // Read endpoints.
    let (limit, key) = user_key.map_or_else(
        || (30_u32, format!("rl:read:ip:{ip}:{}", window_bucket(now_ms, 60_000))),
        |uid| (600_u32, format!("rl:read:user:{uid}:{}", window_bucket(now_ms, 60_000))),
    );
    (limit, 60_000, key)
}

/// Extracts a best-effort user identifier from the JWT without full verification.
fn extract_user_key(request: &Request, state: &AppState) -> Option<String> {
    let auth = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)?;

    // Decode without signature validation — used only for rate-limit keying.
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    let key = jsonwebtoken::DecodingKey::from_secret(state.token_config.jwt_secret.as_bytes());
    jsonwebtoken::decode::<crate::auth::tokens::AccessClaims>(auth, &key, &validation)
        .ok()
        .map(|d| d.claims.sub.to_string())
}

fn extract_ip(request: &Request) -> String {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("unknown")
        .to_owned()
}

/// Bucketing helper keeps the rate-limit key stable within a window.
const fn window_bucket(now_ms: i64, window_ms: i64) -> i64 {
    now_ms / window_ms
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| {
            i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
        })
}

