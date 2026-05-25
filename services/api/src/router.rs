use axum::middleware::from_fn;
use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, post};
use axum::Router;

use crate::extractors::hmac::hmac_auth_middleware;
use crate::handlers::account::{faucet, get_balances, get_history, get_profile};
use crate::handlers::api_keys::{create_api_key, list_api_keys, revoke_api_key};
use crate::handlers::auth::{login, refresh, signup};
use crate::handlers::health::{health, ready};
use crate::handlers::markets::{get_klines, get_market, get_orderbook, get_trades, list_markets};
use crate::handlers::orders::{
    cancel_all_orders, cancel_order, get_order, list_orders, place_order,
};
use crate::handlers::totp::{disable_totp, setup_totp, verify_totp};
use crate::handlers::ws::ws_handler;
use crate::middleware::{not_found, request_id};
use crate::openapi::openapi_handler;
use crate::rate_limit::rate_limit_middleware;
use crate::security_headers::security_headers;
use crate::state::AppState;

/// Builds the full API router (PRD §10.3).
pub fn build_router(state: AppState) -> Router {
    let auth_routes = Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/2fa/setup", post(setup_totp))
        .route("/auth/2fa/verify", post(verify_totp))
        .route("/auth/2fa/disable", post(disable_totp));

    let market_routes = Router::new()
        .route("/markets", get(list_markets))
        .route("/markets/{symbol}", get(get_market))
        .route("/markets/{symbol}/orderbook", get(get_orderbook))
        .route("/markets/{symbol}/trades", get(get_trades))
        .route("/markets/{symbol}/klines", get(get_klines));

    let order_routes = Router::new()
        .route(
            "/orders",
            post(place_order).delete(cancel_all_orders).get(list_orders),
        )
        .route("/orders/{id}", get(get_order).delete(cancel_order));

    let account_routes = Router::new()
        .route("/account", get(get_profile))
        .route("/account/balances", get(get_balances))
        .route("/account/history", get(get_history))
        .route("/account/api-keys", post(create_api_key).get(list_api_keys))
        .route("/account/api-keys/{id}", delete(revoke_api_key))
        .route("/wallet/faucet", post(faucet));

    let api_v1 = Router::new()
        .merge(auth_routes)
        .merge(market_routes)
        .merge(order_routes)
        .merge(account_routes)
        // HMAC middleware verifies X-AETHER-* headers and injects HmacCaller extension.
        .layer(from_fn_with_state(state.clone(), hmac_auth_middleware))
        // Rate limiting applied after request-id so request context is available.
        .layer(from_fn_with_state(state.clone(), rate_limit_middleware));

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/ws", get(ws_handler))
        .route("/openapi.json", get(openapi_handler))
        .nest("/api/v1", api_v1)
        .fallback(not_found)
        .with_state(state)
        .layer(from_fn(security_headers))
        .layer(from_fn(request_id))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use serde_json::Value;
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use time::OffsetDateTime;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::auth::tokens::TokenConfig;
    use crate::auth::AuthService;
    use crate::clock::tests::{FixedClock, FixedIds, NoopDelay};
    use crate::middleware::X_REQUEST_ID;
    use crate::readiness::tests::StaticReadiness;
    use crate::repositories::memory::MemoryAuthRepository;
    use crate::state::AppState;
    use crate::ws::hub::Hub;

    use super::build_router;

    #[tokio::test]
    async fn health_returns_request_id_header() -> TestResult {
        let app = build_router(test_state());
        let request_id = Uuid::from_u128(10);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(X_REQUEST_ID, request_id.to_string())
                    .body(Body::empty())?,
            )
            .await
            .map_err(infallible)?;

        assert_eq!(response.status(), StatusCode::OK);
        let expected_header = request_id.to_string().parse()?;
        assert_eq!(response.headers().get(X_REQUEST_ID), Some(&expected_header));
        Ok(())
    }

    #[tokio::test]
    async fn signup_rejects_weak_password_with_envelope() -> TestResult {
        let app = build_router(test_state());
        let request_id = Uuid::from_u128(11);
        let response = app
            .oneshot(json_request(
                "/api/v1/auth/signup",
                request_id,
                r#"{"email":"user@example.com","password":"weak"}"#,
            ))
            .await
            .map_err(infallible)?;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            string_field(&body, "requestId"),
            Some(request_id.to_string())
        );
        assert_eq!(
            nested_string_field(&body, "error", "code"),
            Some("WEAK_PASSWORD".to_owned())
        );
        Ok(())
    }

    #[tokio::test]
    async fn signup_then_refresh_returns_tokens() -> TestResult {
        let app = build_router(test_state());
        let signup = app
            .clone()
            .oneshot(json_request(
                "/api/v1/auth/signup",
                Uuid::from_u128(12),
                r#"{"email":"user@example.com","password":"Str0ng!Passw0rd"}"#,
            ))
            .await
            .map_err(infallible)?;
        assert_eq!(signup.status(), StatusCode::CREATED);
        let signup_body = response_json(signup).await;
        let refresh_token = string_field(&signup_body, "refreshToken").unwrap_or_default();

        let refresh = app
            .oneshot(json_request(
                "/api/v1/auth/refresh",
                Uuid::from_u128(13),
                &format!(r#"{{"refreshToken":"{refresh_token}"}}"#),
            ))
            .await
            .map_err(infallible)?;
        let refresh_status = refresh.status();
        let refresh_body = response_json(refresh).await;

        assert_eq!(refresh_status, StatusCode::OK);
        assert!(string_field(&refresh_body, "accessToken").is_some_and(|v| !v.is_empty()));
        assert!(string_field(&refresh_body, "refreshToken").is_some_and(|v| !v.is_empty()));
        Ok(())
    }

    fn test_state() -> AppState {
        let repository = MemoryAuthRepository::shared();
        let jwt_secret = "test-secret-that-is-long-enough-for-hs256".to_owned();
        let token_config = Arc::new(TokenConfig {
            jwt_secret,
            access_token_ttl: Duration::from_mins(15),
            mfa_token_ttl: Duration::from_mins(5),
            refresh_token_ttl: Duration::from_hours(720),
        });
        let now = OffsetDateTime::from_unix_timestamp(1_735_689_600)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        let auth = AuthService::new(
            repository,
            (*token_config).clone(),
            FixedClock::new(now),
            Arc::new(NoopDelay),
            FixedIds::new(Uuid::from_u128(42)),
        );
        let engine = crate::engine_client::EngineClient::new(
            "127.0.0.1:7878"
                .parse()
                .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 7878))),
            Duration::from_millis(50),
        );
        AppState::new(
            auth,
            token_config,
            Arc::new(StaticReadiness::ready()),
            PgPoolOptions::new().connect_lazy_with(PgConnectOptions::new()),
            engine,
            Hub::new(),
        )
    }

    fn json_request(path: &str, request_id: Uuid, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(X_REQUEST_ID, request_id.to_string())
            .body(Body::from(body.to_owned()))
            .unwrap_or_else(|_| Request::new(Body::empty()))
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 65_536)
            .await
            .unwrap_or_default();
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }

    fn string_field(body: &Value, field: &str) -> Option<String> {
        body.get(field)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn nested_string_field(body: &Value, parent: &str, field: &str) -> Option<String> {
        body.get(parent)
            .and_then(|value| value.get(field))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn infallible(error: std::convert::Infallible) -> Box<dyn std::error::Error + Send + Sync> {
        match error {}
    }

    type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
