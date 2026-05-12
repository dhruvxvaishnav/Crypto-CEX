//! Security response headers per PRD §18.3.
//!
//! Applied to every response via a tower middleware layer.

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

/// Injects all security headers required by PRD §18.3.
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    macro_rules! set_header {
        ($name:literal, $value:literal) => {
            headers.insert(
                axum::http::header::HeaderName::from_static($name),
                HeaderValue::from_static($value),
            );
        };
    }

    set_header!(
        "strict-transport-security",
        "max-age=63072000; includeSubDomains; preload"
    );
    set_header!(
        "content-security-policy",
        "default-src 'self'; img-src 'self' data: https:; \
         script-src 'self'; connect-src 'self'; \
         frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
    );
    set_header!("x-content-type-options", "nosniff");
    set_header!("referrer-policy", "strict-origin-when-cross-origin");
    set_header!(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()"
    );
    set_header!("cross-origin-opener-policy", "same-origin");
    set_header!("cross-origin-resource-policy", "same-origin");
    // Deny framing everywhere.
    set_header!("x-frame-options", "DENY");

    response
}
