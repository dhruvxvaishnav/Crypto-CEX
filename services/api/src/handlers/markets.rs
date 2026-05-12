use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::errors::{ApiError, ErrorCode};
use crate::middleware::RequestContext;
use crate::repositories::markets as repo;
use crate::state::AppState;
use cex_proto::SnapshotRequest;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketResponse {
    id: Uuid,
    symbol: String,
    base_asset: String,
    quote_asset: String,
    status: String,
    tick_size: String,
    lot_size: String,
    min_notional: String,
    maker_fee_bps: String,
    taker_fee_bps: String,
    last_price: Option<String>,
    volume_24h: Option<String>,
    high_24h: Option<String>,
    low_24h: Option<String>,
    price_change_pct: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TradeResponse {
    id: Uuid,
    price: String,
    qty: String,
    side: String,
    ts: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KlineResponse {
    ts: i64,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderbookResponse {
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
    seq: u64,
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    #[serde(default = "default_trade_limit")]
    limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct KlinesQuery {
    interval: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    #[serde(default = "default_kline_limit")]
    limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct OrderbookQuery {
    #[serde(default = "default_depth")]
    depth: usize,
}

const fn default_trade_limit() -> i64 {
    50
}
const fn default_kline_limit() -> i64 {
    500
}
const fn default_depth() -> usize {
    100
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /markets`
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure.
pub async fn list_markets(
    State(state): State<AppState>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let rows = repo::list_markets(&state.db)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "markets.list.db_error");
            ApiError::internal()
        })?;

    let body: Vec<MarketResponse> = rows.into_iter().map(market_row_to_response).collect();
    Ok(Json(serde_json::json!({ "data": body })))
}

/// `GET /markets/:symbol`
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure or unknown symbol.
pub async fn get_market(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let row = repo::get_market(&state.db, &symbol.to_uppercase())
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "markets.get.db_error");
            ApiError::internal()
        })?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, ErrorCode::MarketUnknown, "Market not found")
        })?;

    Ok(Json(market_row_to_response(row)))
}

/// `GET /markets/:symbol/orderbook?depth=100`
///
/// # Errors
///
/// Returns [`ApiError`] when the engine is unavailable or the market is unknown.
pub async fn get_orderbook(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(q): Query<OrderbookQuery>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let depth = q.depth.clamp(1, 500);
    let snap = state
        .engine
        .snapshot(SnapshotRequest {
            request_id: ctx.request_id,
            symbol: symbol.to_uppercase(),
            depth,
        })
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::EngineTimeout,
                "Engine unavailable",
            )
        })?;

    Ok(Json(OrderbookResponse {
        bids: price_levels(snap.bids),
        asks: price_levels(snap.asks),
        seq: snap.seq,
    }))
}

/// `GET /markets/:symbol/trades?limit=50`
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure.
pub async fn get_trades(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(q): Query<TradesQuery>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.clamp(1, 1000);
    let rows = repo::recent_trades(&state.db, &symbol.to_uppercase(), limit)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "markets.trades.db_error");
            ApiError::internal()
        })?;

    let body: Vec<TradeResponse> = rows
        .into_iter()
        .map(|r| TradeResponse {
            id: r.id,
            price: r.price.to_string(),
            qty: r.quantity.to_string(),
            side: r.taker_side,
            ts: r.created_at.to_string(),
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": body })))
}

/// `GET /markets/:symbol/klines?interval=1m&from=<unix_s>&to=<unix_s>&limit=500`
///
/// # Errors
///
/// Returns [`ApiError`] on DB failure or bad query params.
pub async fn get_klines(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(q): Query<KlinesQuery>,
    axum::extract::Extension(ctx): axum::extract::Extension<RequestContext>,
) -> Result<impl IntoResponse, ApiError> {
    let interval = q.interval.as_deref().unwrap_or("1m");
    let valid_intervals = ["1m", "5m", "15m", "1h", "4h", "1d"];
    if !valid_intervals.contains(&interval) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::Internal,
            "Invalid interval; use 1m|5m|15m|1h|4h|1d",
        ));
    }

    let from = q
        .from
        .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let to = q
        .to
        .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
        .unwrap_or_else(OffsetDateTime::now_utc);
    let limit = q.limit.clamp(1, 1000);

    let rows = repo::klines(&state.db, &symbol.to_uppercase(), interval, from, to, limit)
        .await
        .map_err(|err| {
            tracing::error!(err = %err, request_id = %ctx.request_id, "markets.klines.db_error");
            ApiError::internal()
        })?;

    let body: Vec<KlineResponse> = rows
        .into_iter()
        .map(|r| KlineResponse {
            ts: r.ts.unix_timestamp(),
            open: r.open.to_string(),
            high: r.high.to_string(),
            low: r.low.to_string(),
            close: r.close.to_string(),
            volume: r.volume.to_string(),
        })
        .collect();

    Ok(Json(serde_json::json!({ "data": body })))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn market_row_to_response(row: repo::MarketRow) -> MarketResponse {
    MarketResponse {
        id: row.id,
        symbol: row.symbol,
        base_asset: row.base_asset_symbol,
        quote_asset: row.quote_asset_symbol,
        status: row.status,
        tick_size: row.tick_size.to_string(),
        lot_size: row.lot_size.to_string(),
        min_notional: row.min_notional.to_string(),
        maker_fee_bps: row.maker_fee_bps.to_string(),
        taker_fee_bps: row.taker_fee_bps.to_string(),
        last_price: row.last_price.map(|d| d.to_string()),
        volume_24h: row.volume_24h.map(|d| d.to_string()),
        high_24h: row.high_24h.map(|d| d.to_string()),
        low_24h: row.low_24h.map(|d| d.to_string()),
        price_change_pct: row.price_change_pct.map(|d| d.to_string()),
    }
}

fn price_levels(levels: Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>) -> Vec<[String; 2]> {
    levels
        .into_iter()
        .map(|(p, q)| [p.to_string(), q.to_string()])
        .collect()
}
