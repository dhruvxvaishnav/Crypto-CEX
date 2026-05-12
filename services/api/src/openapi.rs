//! OpenAPI 3.1 specification for the Aether API (PRD §10.5).
//!
//! The spec is generated via the `utoipa` crate. New handlers should be
//! annotated with `#[utoipa::path(...)]` and registered in [`AetherApiDoc`].

use axum::http::StatusCode;
use axum::response::IntoResponse;
use utoipa::OpenApi;

/// Aether Exchange REST API — OpenAPI 3.1 document.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Aether Exchange API",
        version = "1.0.0",
        description = "Portfolio-demonstration crypto spot exchange. \
                       Not a real exchange — demo funds only.",
        contact(name = "Aether Team")
    ),
    tags(
        (name = "auth",    description = "Authentication — signup, login, token rotation, 2FA"),
        (name = "markets", description = "Market data — order book, trades, klines"),
        (name = "orders",  description = "Order management — place, cancel, list"),
        (name = "account", description = "Account — profile, balances, ledger, API keys"),
        (name = "wallet",  description = "Wallet — faucet deposits"),
        (name = "system",  description = "Health and readiness probes"),
    ),
    servers(
        (url = "/api/v1", description = "Current version")
    ),
    components(
        schemas(
            ErrorEnvelopeSchema,
            AuthTokensSchema,
            PlaceOrderSchema,
            CreateApiKeySchema,
            VerifyTotpSchema,
        )
    )
)]
pub struct AetherApiDoc;

// ── Schema stubs for utoipa component registry ────────────────────────────────

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ErrorEnvelopeSchema {
    error: ErrorBodySchema,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    request_id: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ErrorBodySchema {
    #[schema(example = "INVALID_CREDENTIALS")]
    code: String,
    #[schema(example = "Invalid credentials")]
    message: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct AuthTokensSchema {
    #[schema(example = "eyJhbGci...")]
    access_token: String,
    #[schema(example = "v1_abc123...")]
    refresh_token: String,
    #[schema(example = "Bearer")]
    token_type: String,
    #[schema(example = 900)]
    expires_in: u64,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct PlaceOrderSchema {
    #[schema(example = "order-abc-123")]
    client_order_id: Option<String>,
    #[schema(example = "BTCUSDT")]
    market: String,
    #[schema(example = "buy")]
    side: String,
    #[schema(example = "limit", rename = "type")]
    order_type: String,
    #[schema(example = "50000.00")]
    price: Option<String>,
    #[schema(example = "0.001")]
    quantity: Option<String>,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct CreateApiKeySchema {
    #[schema(example = "my trading bot")]
    label: String,
    #[schema(example = json!(["read", "trade"]))]
    permissions: Vec<String>,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct VerifyTotpSchema {
    #[schema(example = "123456")]
    code: String,
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// `GET /openapi.json` — Serve the OpenAPI specification.
///
/// # Errors
///
/// Returns 500 if serialisation of the spec fails (should never happen in practice).
pub async fn openapi_handler() -> impl IntoResponse {
    match AetherApiDoc::openapi().to_json() {
        Ok(json) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
