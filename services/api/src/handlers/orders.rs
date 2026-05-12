use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cex_proto::{
    CancelAllRequest, CancelRequest, EngineOrder, EngineOrderType, OrderSide, PlaceRequest,
    StpMode, TradeFill,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::engine_client::EngineClientError;
use crate::errors::{ApiError, ErrorCode};
use crate::extractors::auth::AuthenticatedUser;
use crate::middleware::RequestContext;
use crate::repositories::markets as market_repo;
use crate::repositories::orders as order_repo;
use crate::state::AppState;

// ── Input schema (PRD §10.4) ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceOrderInput {
    pub client_order_id: Option<String>,
    pub market: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub price: Option<String>,
    pub stop_price: Option<String>,
    pub quantity: Option<String>,
    pub quote_quantity: Option<String>,
    pub display_quantity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListOrdersQuery {
    pub status: Option<String>,
    pub market: Option<String>,
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct CancelAllQuery {
    pub market: Option<String>,
}

const fn default_limit() -> i64 {
    50
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponse {
    id: Uuid,
    client_order_id: Option<String>,
    market: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    status: String,
    price: Option<String>,
    stop_price: Option<String>,
    quantity: Option<String>,
    quote_quantity: Option<String>,
    filled_quantity: String,
    avg_fill_price: Option<String>,
    fills: Vec<FillResponse>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FillResponse {
    trade_id: Uuid,
    price: String,
    quantity: String,
    side: String,
    ts: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `POST /orders` — Place an order (PRD §FR-TRADE-01).
#[allow(clippy::too_many_lines)]
///
/// # Errors
///
/// Returns [`ApiError`] on validation failure, insufficient balance,
/// market issues, or engine rejection.
pub async fn place_order(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
    payload: Result<Json<PlaceOrderInput>, axum::extract::rejection::JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = payload.map_err(|_| {
        ApiError::new(StatusCode::BAD_REQUEST, ErrorCode::Internal, "Invalid JSON body")
            .with_request_id(ctx.request_id)
    })?;

    let symbol = input.market.to_uppercase();

    // ── AC2: Idempotency check ────────────────────────────────────────────────
    if let Some(ref cid) = input.client_order_id {
        if let Some(existing) = order_repo::find_by_client_order_id(&state.db, caller.user_id, cid)
            .await
            .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        {
            return Ok((StatusCode::OK, Json(order_to_response(existing, vec![]))));
        }
    }

    // ── AC3: Validate inputs ──────────────────────────────────────────────────
    let (engine_side, engine_type) = parse_side_type(&input.side, &input.order_type, ctx.request_id)?;
    let price = parse_optional_decimal(input.price.as_deref(), "price", ctx.request_id)?;
    let stop_price = parse_optional_decimal(input.stop_price.as_deref(), "stopPrice", ctx.request_id)?;
    let quantity = parse_optional_decimal(input.quantity.as_deref(), "quantity", ctx.request_id)?;
    let quote_quantity = parse_optional_decimal(input.quote_quantity.as_deref(), "quoteQuantity", ctx.request_id)?;
    let display_quantity = parse_optional_decimal(input.display_quantity.as_deref(), "displayQuantity", ctx.request_id)?;

    // Quantity: limit/ioc/fok/post_only need `quantity`; market buy needs `quoteQuantity`.
    let effective_qty = match engine_type {
        EngineOrderType::Market if engine_side == OrderSide::Buy => {
            quote_quantity.ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    ErrorCode::Internal,
                    "Market buy requires quoteQuantity",
                )
                .with_request_id(ctx.request_id)
            })?
        }
        EngineOrderType::Market => quantity.ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Internal,
                "Market sell requires quantity",
            )
            .with_request_id(ctx.request_id)
        })?,
        _ => quantity.ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Internal,
                "quantity is required for this order type",
            )
            .with_request_id(ctx.request_id)
        })?,
    };

    if effective_qty <= Decimal::ZERO {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "quantity must be positive",
        )
        .with_request_id(ctx.request_id));
    }

    // Limit price validation.
    if matches!(engine_type, EngineOrderType::Limit | EngineOrderType::PostOnly | EngineOrderType::Ioc | EngineOrderType::Fok) && price.is_none() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "price is required for this order type",
        )
        .with_request_id(ctx.request_id));
    }

    // ── Market validation ─────────────────────────────────────────────────────
    let (market_id, base_asset_id, quote_asset_id, market_status) =
        market_repo::get_market_assets(&state.db, &symbol)
            .await
            .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
            .ok_or_else(|| {
                ApiError::new(StatusCode::NOT_FOUND, ErrorCode::MarketUnknown, "Market not found")
                    .with_request_id(ctx.request_id)
            })?;

    if market_status == "halted" || market_status == "delisted" {
        return Err(
            ApiError::new(StatusCode::CONFLICT, ErrorCode::MarketHalted, "Market is halted")
                .with_request_id(ctx.request_id),
        );
    }

    // ── AC3: Balance lock ─────────────────────────────────────────────────────
    // Stop orders are exempt: balance is locked when the trigger fires.
    let (lock_asset, lock_amount) = balance_lock_params(
        engine_side,
        engine_type,
        price,
        effective_qty,
        quote_quantity,
        base_asset_id,
        quote_asset_id,
    );

    if let (Some(asset_id), Some(amount)) = (lock_asset, lock_amount) {
        let locked = order_repo::try_lock_balance(&state.db, caller.user_id, asset_id, amount)
            .await
            .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

        if !locked {
            return Err(ApiError::new(
                StatusCode::PAYMENT_REQUIRED,
                ErrorCode::InsufficientBalance,
                "Insufficient balance",
            )
            .with_request_id(ctx.request_id));
        }
    }

    let order_id = Uuid::new_v4();

    // ── Insert pending order ──────────────────────────────────────────────────
    order_repo::insert(
        &state.db,
        order_repo::InsertOrder {
            id: order_id,
            user_id: caller.user_id,
            market_id,
            client_order_id: input.client_order_id.clone(),
            side: input.side.to_lowercase(),
            order_type: order_type_str(engine_type),
            status: "pending".to_owned(),
            price,
            stop_price,
            quantity: Some(effective_qty),
            quote_quantity,
            display_quantity,
        },
    )
    .await
    .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?;

    // ── AC4: Forward to engine ────────────────────────────────────────────────
    let place_req = PlaceRequest {
        request_id: ctx.request_id,
        order: EngineOrder {
            id: order_id,
            user_id: caller.user_id,
            symbol: symbol.clone(),
            side: engine_side,
            order_type: engine_type,
            price,
            quantity: effective_qty,
            stp_mode: StpMode::default(),
            stop_price,
            oco_linked_id: None,
            display_qty: display_quantity,
        },
    };

    let ack = state.engine.place(place_req).await.map_err(|err| {
        // On engine failure, attempt to roll back the balance lock.
        if let (Some(asset_id), Some(amount)) = (lock_asset, lock_amount) {
            let pool = state.db.clone();
            let user_id = caller.user_id;
            tokio::spawn(async move {
                if let Err(e) = order_repo::unlock_balance(&pool, user_id, asset_id, amount).await
                {
                    tracing::error!(err = %e, "orders.place.unlock_failed_after_engine_error");
                }
            });
        }
        match err {
            EngineClientError::Timeout => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::EngineTimeout,
                "Engine did not respond in time",
            )
            .with_request_id(ctx.request_id),
            EngineClientError::Rejected { ref code, .. } => map_engine_reject(code, ctx.request_id),
            _ => ApiError::internal().with_request_id(ctx.request_id),
        }
    })?;

    // Update order status from ack.
    let status = format!("{:?}", ack.status).to_lowercase();
    let _ = order_repo::update_status(&state.db, order_id, &status).await;

    let order_row = order_repo::find_by_id(&state.db, order_id, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .ok_or_else(|| ApiError::internal().with_request_id(ctx.request_id))?;

    tracing::info!(
        event = "engine.place.ack",
        order_id = %order_id,
        status = %status,
        fills = %ack.fills.len(),
        request_id = %ctx.request_id
    );

    Ok((
        StatusCode::CREATED,
        Json(order_to_response(order_row, ack.fills)),
    ))
}

/// `GET /orders/:id` — Get a single order.
///
/// # Errors
///
/// Returns [`ApiError`] when the order is not found or DB fails.
pub async fn get_order(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(order_id): Path<Uuid>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let row = order_repo::find_by_id(&state.db, order_id, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::OrderNotFound, "Order not found")
                .with_request_id(ctx.request_id)
        })?;

    Ok(Json(order_to_response(row, vec![])))
}

/// `GET /orders` — List orders with optional filters.
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure.
pub async fn list_orders(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Query(q): Query<ListOrdersQuery>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let cursor = q
        .cursor
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok());

    let limit = q.limit.clamp(1, 200);

    let rows = order_repo::list(
        &state.db,
        caller.user_id,
        q.status.as_deref(),
        q.market.as_deref(),
        cursor,
        limit,
    )
    .await
    .map_err(|err| {
        tracing::error!(err = %err, request_id = %ctx.request_id, "orders.list.db_error");
        ApiError::internal().with_request_id(ctx.request_id)
    })?;

    let next_cursor = rows
        .last()
        .map(|r| r.created_at.unix_timestamp().to_string());

    let body: Vec<OrderResponse> = rows
        .into_iter()
        .map(|r| order_to_response(r, vec![]))
        .collect();

    Ok(Json(serde_json::json!({
        "data": body,
        "nextCursor": next_cursor
    })))
}

/// `DELETE /orders/:id` — Cancel a single order.
///
/// # Errors
///
/// Returns [`ApiError`] when the order is not found, already final, or engine rejects.
pub async fn cancel_order(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(order_id): Path<Uuid>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let row = order_repo::find_by_id(&state.db, order_id, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::OrderNotFound, "Order not found")
                .with_request_id(ctx.request_id)
        })?;

    // Idempotent: already final orders return as-is.
    if matches!(row.status.as_str(), "filled" | "canceled" | "rejected") {
        return Ok((StatusCode::OK, Json(order_to_response(row, vec![]))));
    }

    let ack = state
        .engine
        .cancel(CancelRequest {
            request_id: ctx.request_id,
            symbol: row.symbol.clone(),
            order_id,
        })
        .await
        .map_err(|err| match err {
            EngineClientError::Timeout => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::EngineTimeout,
                "Engine did not respond in time",
            )
            .with_request_id(ctx.request_id),
            _ => ApiError::internal().with_request_id(ctx.request_id),
        })?;

    let status = format!("{:?}", ack.status).to_lowercase();
    let _ = order_repo::update_status(&state.db, order_id, &status).await;

    let updated = order_repo::find_by_id(&state.db, order_id, caller.user_id)
        .await
        .map_err(|_| ApiError::internal().with_request_id(ctx.request_id))?
        .unwrap_or(row);

    Ok((StatusCode::OK, Json(order_to_response(updated, vec![]))))
}

/// `DELETE /orders[?market=BTCUSDT]` — Cancel all open orders.
///
/// # Errors
///
/// Returns [`ApiError`] on engine failure.
pub async fn cancel_all_orders(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Query(q): Query<CancelAllQuery>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let ack = state
        .engine
        .cancel_all(CancelAllRequest {
            request_id: ctx.request_id,
            user_id: caller.user_id,
            symbol: q.market.map(|s| s.to_uppercase()),
        })
        .await
        .map_err(|err| match err {
            EngineClientError::Timeout => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::EngineTimeout,
                "Engine did not respond in time",
            )
            .with_request_id(ctx.request_id),
            _ => ApiError::internal().with_request_id(ctx.request_id),
        })?;

    tracing::info!(
        event = "orders.cancel_all",
        user_id = %caller.user_id,
        count = %ack.order_ids.len()
    );

    Ok(Json(serde_json::json!({
        "canceledOrderIds": ack.order_ids,
        "count": ack.order_ids.len()
    })))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_side_type(
    side: &str,
    order_type: &str,
    request_id: Uuid,
) -> Result<(OrderSide, EngineOrderType), ApiError> {
    let engine_side = match side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Internal,
                "side must be buy or sell",
            )
            .with_request_id(request_id))
        }
    };
    let engine_type = match order_type.to_lowercase().as_str() {
        "limit" => EngineOrderType::Limit,
        "market" => EngineOrderType::Market,
        "ioc" => EngineOrderType::Ioc,
        "fok" => EngineOrderType::Fok,
        "post_only" => EngineOrderType::PostOnly,
        "stop_limit" => EngineOrderType::StopLimit,
        "stop_market" => EngineOrderType::StopMarket,
        "oco" => EngineOrderType::Oco,
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Internal,
                "Unknown order type",
            )
            .with_request_id(request_id))
        }
    };
    Ok((engine_side, engine_type))
}

fn parse_optional_decimal(
    raw: Option<&str>,
    field: &str,
    request_id: Uuid,
) -> Result<Option<Decimal>, ApiError> {
    match raw {
        None => Ok(None),
        Some(s) => s.parse::<Decimal>().map(Some).map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::Internal,
                format!("Invalid decimal for {field}"),
            )
            .with_request_id(request_id)
        }),
    }
}

/// Determines which asset to lock and by how much before forwarding to engine.
///
/// Stop orders are excluded — the engine locks balance when the trigger fires.
#[allow(clippy::too_many_arguments)]
fn balance_lock_params(
    side: OrderSide,
    order_type: EngineOrderType,
    price: Option<Decimal>,
    quantity: Decimal,
    quote_quantity: Option<Decimal>,
    base_asset_id: Uuid,
    quote_asset_id: Uuid,
) -> (Option<Uuid>, Option<Decimal>) {
    match (side, order_type) {
        // Stop orders: no lock until trigger.
        (_, EngineOrderType::StopLimit | EngineOrderType::StopMarket) => (None, None),
        // BUY limit: lock quote = price × qty
        (OrderSide::Buy, _) => {
            if let Some(p) = price {
                let amount = p * quantity;
                (Some(quote_asset_id), Some(amount))
            } else if let Some(qq) = quote_quantity {
                (Some(quote_asset_id), Some(qq))
            } else {
                (None, None)
            }
        }
        // SELL: lock base qty.
        (OrderSide::Sell, _) => (Some(base_asset_id), Some(quantity)),
    }
}

fn order_type_str(t: EngineOrderType) -> String {
    match t {
        EngineOrderType::Limit => "limit",
        EngineOrderType::Market => "market",
        EngineOrderType::Ioc => "ioc",
        EngineOrderType::Fok => "fok",
        EngineOrderType::PostOnly => "post_only",
        EngineOrderType::StopLimit => "stop_limit",
        EngineOrderType::StopMarket => "stop_market",
        EngineOrderType::Oco => "oco",
        EngineOrderType::Iceberg => "limit",
    }
    .to_owned()
}

fn order_to_response(row: order_repo::OrderRow, fills: Vec<TradeFill>) -> OrderResponse {
    OrderResponse {
        id: row.id,
        client_order_id: row.client_order_id,
        market: row.symbol,
        side: row.side,
        order_type: row.order_type,
        status: row.status,
        price: row.price.map(|d| d.to_string()),
        stop_price: row.stop_price.map(|d| d.to_string()),
        quantity: row.quantity.map(|d| d.to_string()),
        quote_quantity: row.quote_quantity.map(|d| d.to_string()),
        filled_quantity: row.filled_quantity.to_string(),
        avg_fill_price: row.avg_fill_price.map(|d| d.to_string()),
        fills: fills
            .into_iter()
            .map(|f| FillResponse {
                trade_id: f.trade_id,
                price: f.price.to_string(),
                quantity: f.quantity.to_string(),
                side: format!("{:?}", f.taker_side).to_lowercase(),
                ts: f.ts,
            })
            .collect(),
        created_at: row.created_at.to_string(),
        updated_at: row.updated_at.to_string(),
    }
}

fn map_engine_reject(code: &str, request_id: Uuid) -> ApiError {
    let (http_status, error_code, msg) = match code {
        "POST_ONLY_REJECTED" => (
            StatusCode::CONFLICT,
            ErrorCode::PostOnlyRejected,
            "Post-only order would cross the book",
        ),
        "FOK_NOT_FILLED" => (
            StatusCode::CONFLICT,
            ErrorCode::FokNotFilled,
            "FOK order could not be fully filled",
        ),
        "SELF_TRADE_PREVENTED" => (
            StatusCode::CONFLICT,
            ErrorCode::SelfTradePrevented,
            "Self-trade prevented",
        ),
        "STOP_PRICE_INVALID" => (
            StatusCode::BAD_REQUEST,
            ErrorCode::StopPriceInvalid,
            "Stop price is on the wrong side of last price",
        ),
        "MARKET_HALTED" => (
            StatusCode::CONFLICT,
            ErrorCode::MarketHalted,
            "Market is halted",
        ),
        _ => (
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "Order rejected by engine",
        ),
    };
    ApiError::new(http_status, error_code, msg).with_request_id(request_id)
}
