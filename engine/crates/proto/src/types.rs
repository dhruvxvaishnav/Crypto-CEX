use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// API-to-engine request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EngineRequest {
    /// Submit an order to the matching engine.
    #[serde(rename = "place")]
    Place(PlaceRequest),
    /// Cancel a single resting order.
    #[serde(rename = "cancel")]
    Cancel(CancelRequest),
    /// Cancel all resting orders for a user, optionally scoped to one symbol.
    #[serde(rename = "cancelAll")]
    CancelAll(CancelAllRequest),
    /// Halt a market so new orders are rejected.
    #[serde(rename = "haltMarket")]
    HaltMarket(MarketControlRequest),
    /// Resume a halted market.
    #[serde(rename = "resumeMarket")]
    ResumeMarket(MarketControlRequest),
    /// Return an L2 order-book snapshot.
    #[serde(rename = "snapshot")]
    Snapshot(SnapshotRequest),
    /// Health-check the engine TCP session.
    #[serde(rename = "ping")]
    Ping(PingRequest),
}

/// Engine-to-API response envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EngineResponse {
    /// Request succeeded.
    #[serde(rename = "ack")]
    Ack(AckResponse),
    /// Request was rejected with a stable error code.
    #[serde(rename = "reject")]
    Reject(RejectResponse),
    /// Unsolicited engine event broadcast.
    #[serde(rename = "event")]
    Event(SequencedEngineEvent),
    /// Response to a ping request.
    #[serde(rename = "pong")]
    Pong(PongResponse),
}

/// Order payload accepted by the matching engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineOrder {
    /// Unique public order ID.
    pub id: Uuid,
    /// Owning user ID.
    pub user_id: Uuid,
    /// Market symbol, for example `BTCUSDT`.
    pub symbol: String,
    /// Buy or sell side.
    pub side: OrderSide,
    /// Order type.
    pub order_type: EngineOrderType,
    /// Optional limit price, required by limit-like order types.
    pub price: Option<Decimal>,
    /// Total order quantity.
    pub quantity: Decimal,
    /// Self-trade-prevention mode.
    #[serde(default)]
    pub stp_mode: StpMode,
    /// Optional stop trigger price.
    pub stop_price: Option<Decimal>,
    /// Optional linked leg for OCO orders.
    pub oco_linked_id: Option<Uuid>,
    /// Optional visible peak for iceberg orders.
    pub display_qty: Option<Decimal>,
}

/// Side of an order or fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    /// Buy side.
    Buy,
    /// Sell side.
    Sell,
}

/// Wire order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineOrderType {
    /// Limit order.
    Limit,
    /// Market order.
    Market,
    /// Immediate-or-cancel order.
    Ioc,
    /// Fill-or-kill order.
    Fok,
    /// Post-only order.
    PostOnly,
    /// Stop-limit order.
    StopLimit,
    /// Stop-market order.
    StopMarket,
    /// One-cancels-other order.
    Oco,
    /// Iceberg order.
    Iceberg,
}

/// Self-trade-prevention mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StpMode {
    /// Decrement and cancel the smaller side.
    #[default]
    Decrement,
    /// Cancel the resting maker order.
    CancelMaker,
    /// Cancel the incoming taker order.
    CancelTaker,
}

/// Stable order status values returned in ack responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order is resting with no fills.
    New,
    /// Order is resting with partial fills.
    Partial,
    /// Order filled completely.
    Filled,
    /// Order was canceled.
    Canceled,
    /// Order was rejected before entering the book.
    Rejected,
}

/// A price level represented as `[price, quantity]` in JSON.
pub type PriceLevel = (Decimal, Decimal);

/// Request body for `kind: "place"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRequest {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// Order to submit.
    pub order: EngineOrder,
}

/// Request body for `kind: "cancel"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// Market symbol.
    pub symbol: String,
    /// Order ID to cancel.
    pub order_id: Uuid,
}

/// Request body for `kind: "cancelAll"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllRequest {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// User whose open orders should be canceled.
    pub user_id: Uuid,
    /// Optional market symbol filter.
    pub symbol: Option<String>,
}

/// Request body for market halt/resume commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketControlRequest {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// Market symbol.
    pub symbol: String,
}

/// Request body for `kind: "snapshot"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRequest {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// Market symbol.
    pub symbol: String,
    /// Maximum number of bid/ask levels to return.
    pub depth: usize,
}

/// Request body for `kind: "ping"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
}

/// Successful request response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckResponse {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// Successful result payload.
    pub result: AckResult,
}

/// Successful result variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AckResult {
    /// Order placement result.
    #[serde(rename = "order")]
    Order(OrderAck),
    /// Cancel result.
    #[serde(rename = "cancel")]
    Cancel(CancelAck),
    /// Cancel-all result.
    #[serde(rename = "cancelAll")]
    CancelAll(CancelAllAck),
    /// Snapshot result.
    #[serde(rename = "snapshot")]
    Snapshot(BookSnapshot),
    /// Market status result.
    #[serde(rename = "marketStatus")]
    MarketStatus(MarketStatusAck),
}

/// Order placement ack payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAck {
    /// Submitted order ID.
    pub order_id: Uuid,
    /// Resulting order status.
    pub status: OrderStatus,
    /// Initial fills produced by this order.
    pub fills: Vec<TradeFill>,
    /// Last emitted engine event sequence for this request.
    pub seq: u64,
}

/// Single-order cancel ack payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAck {
    /// Canceled order ID.
    pub order_id: Uuid,
    /// Resulting order status.
    pub status: OrderStatus,
    /// Last emitted engine event sequence for this request.
    pub seq: u64,
}

/// Cancel-all ack payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllAck {
    /// Canceled order IDs.
    pub order_ids: Vec<Uuid>,
    /// Last emitted engine event sequence for this request.
    pub seq: u64,
}

/// L2 order-book snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSnapshot {
    /// Market symbol.
    pub symbol: String,
    /// Bid levels, best first.
    pub bids: Vec<PriceLevel>,
    /// Ask levels, best first.
    pub asks: Vec<PriceLevel>,
    /// Current engine event sequence.
    pub seq: u64,
}

/// Market status ack payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusAck {
    /// Market symbol.
    pub symbol: String,
    /// Current market status.
    pub status: MarketStatus,
    /// Current engine event sequence.
    pub seq: u64,
}

/// Rejected request response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectResponse {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
    /// Stable error code from the API error-code set.
    pub code: String,
    /// Safe human-readable message.
    pub message: String,
}

/// Ping response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PongResponse {
    /// Correlation ID supplied by the API process.
    pub request_id: Uuid,
}

/// Engine event with its global sequence number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencedEngineEvent {
    /// Global event sequence.
    pub seq: u64,
    /// Event payload.
    pub event: EngineEvent,
}

/// Broadcast event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EngineEvent {
    /// An order was accepted by the engine.
    #[serde(rename = "orderAccepted")]
    OrderAccepted(OrderAcceptedEvent),
    /// An accepted order rested on the book.
    #[serde(rename = "orderRested")]
    OrderRested(OrderRestedEvent),
    /// A fill occurred.
    #[serde(rename = "fill")]
    Fill(TradeFill),
    /// An order was canceled.
    #[serde(rename = "orderCanceled")]
    OrderCanceled(OrderCanceledEvent),
    /// A book update occurred.
    #[serde(rename = "bookDelta")]
    BookDelta(BookDeltaEvent),
    /// A market status changed.
    #[serde(rename = "marketStatus")]
    MarketStatus(MarketStatusEvent),
}

/// Order-accepted event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAcceptedEvent {
    /// Accepted order.
    pub order: EngineOrder,
}

/// Order-rested event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRestedEvent {
    /// Resting order ID.
    pub order_id: Uuid,
    /// Remaining quantity.
    pub remaining: Decimal,
}

/// Trade fill payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeFill {
    /// Public trade ID.
    pub trade_id: Uuid,
    /// Market symbol.
    pub symbol: String,
    /// Taker order ID.
    pub taker_order_id: Uuid,
    /// Maker order ID.
    pub maker_order_id: Uuid,
    /// Fill price.
    pub price: Decimal,
    /// Fill quantity.
    pub quantity: Decimal,
    /// Taker side.
    pub taker_side: OrderSide,
    /// UTC timestamp in RFC3339 format.
    pub ts: String,
}

/// Order-canceled event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderCanceledEvent {
    /// Canceled order ID.
    pub order_id: Uuid,
    /// Stable cancellation reason.
    pub reason: CancelReason,
}

/// Book-delta event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookDeltaEvent {
    /// Market symbol.
    pub symbol: String,
    /// Bid levels, best first.
    pub bids: Vec<PriceLevel>,
    /// Ask levels, best first.
    pub asks: Vec<PriceLevel>,
}

/// Market-status event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusEvent {
    /// Market symbol.
    pub symbol: String,
    /// New market status.
    pub status: MarketStatus,
}

/// Stable cancellation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelReason {
    /// No liquidity was available.
    NoLiquidity,
    /// FOK order could not be filled completely.
    FokUnfilled,
    /// Post-only order would cross.
    PostOnlyCrossed,
    /// Self-trade prevention canceled an order.
    SelfTradePrevented,
    /// OCO linked leg was canceled.
    Oco,
    /// Administrative cancel.
    AdminCancel,
}

/// Market trading status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketStatus {
    /// Market accepts orders.
    Trading,
    /// Market rejects new orders.
    Halted,
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use serde_json::json;

    use crate::{EngineOrder, EngineOrderType, EngineRequest, OrderSide, PlaceRequest, StpMode};

    #[test]
    fn serialises_prd_request_shape() {
        let request_id = uuid::Uuid::nil();
        let order = EngineOrder {
            id: uuid::Uuid::nil(),
            user_id: uuid::Uuid::nil(),
            symbol: "BTCUSDT".to_owned(),
            side: OrderSide::Buy,
            order_type: EngineOrderType::Limit,
            price: Some(Decimal::new(100, 0)),
            quantity: Decimal::new(2, 0),
            stp_mode: StpMode::Decrement,
            stop_price: None,
            oco_linked_id: None,
            display_qty: None,
        };
        let request = EngineRequest::Place(PlaceRequest { request_id, order });

        let value = serde_json::to_value(request).expect("request serialises");

        assert_eq!(value["kind"], json!("place"));
        assert_eq!(value["requestId"], json!(request_id.to_string()));
        assert_eq!(value["order"]["orderType"], json!("limit"));
        assert_eq!(value["order"]["price"], json!("100"));
        assert_eq!(value["order"]["side"], json!("buy"));
    }
}
