use std::sync::Arc;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Side of the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Buy side (bid).
    Buy,
    /// Sell side (ask).
    Sell,
}

impl Side {
    /// Returns the opposite side.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }
}

/// Type of the order — maps 1:1 to PRD §7.3 FR-TRADE-03 order types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Rests on the book at the specified price; matches at that price or better.
    Limit,
    /// Matches immediately at any price; remaining is cancelled.
    Market,
    /// Immediate-Or-Cancel: matches what it can at the limit price; remaining is cancelled.
    Ioc,
    /// Fill-Or-Kill: must fill entirely or reject with no state change (PRD §14.4).
    Fok,
    /// Post-Only: rests on book only if it would not cross; otherwise rejected (PRD §14.4).
    PostOnly,
    /// Inactive until `stop_price` is hit by a trade; then activates as `Limit` at `price`.
    StopLimit,
    /// Inactive until `stop_price` is hit by a trade; then activates as `Market`.
    StopMarket,
    /// One-Cancels-Other: two-leg order; when one leg terminates, the other is cancelled.
    Oco,
    /// Shows only `display_qty` on the book at once; refreshes display after each peak fills.
    Iceberg,
}

impl OrderType {
    /// Returns `true` for order types that live in the stop registry until triggered.
    #[must_use]
    pub const fn is_stop(self) -> bool {
        matches!(self, Self::StopLimit | Self::StopMarket)
    }

    /// Returns `true` for order types that cancel remaining quantity after matching.
    #[must_use]
    pub const fn cancels_remaining(self) -> bool {
        matches!(self, Self::Market | Self::Ioc)
    }
}

/// Self-trade prevention mode — applied when taker and maker share the same `user_id`.
///
/// The taker's STP mode governs which action is taken (PRD §14.3, FR-TRADE-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StpMode {
    /// Cancel the smaller side; decrement the larger by the smaller's quantity.
    /// If equal, both are cancelled.
    #[default]
    Decrement,
    /// Cancel the resting maker order; taker continues to the next level.
    CancelMaker,
    /// Cancel the incoming taker order; maker stays on the book.
    CancelTaker,
}

/// Stable cancellation reason codes — used in conformance tests and error responses.
///
/// Maps to the error codes in `packages/shared/src/errors.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancelReason {
    /// Market or IOC order found no liquidity.
    NoLiquidity,
    /// FOK order could not be fully filled; rejected without any state change.
    FokUnfilled,
    /// Post-only order would have crossed the book on entry.
    PostOnlyCrossed,
    /// Self-trade prevention triggered.
    SelfTradePrevented,
    /// The other leg of an OCO pair was filled or cancelled first.
    Oco,
    /// Administrative cancel (circuit breaker, cancel-all, etc.).
    AdminCancel,
}

/// Core order representation passed into the matching engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier (UUID v4).
    pub id: Uuid,
    /// Owning user — used for STP detection.
    pub user_id: Uuid,
    /// Market symbol (e.g. `"BTCUSDT"`). `Arc<str>` so cloning an order is O(1) for this field.
    pub symbol: Arc<str>,
    /// Order side.
    pub side: Side,
    /// Order type.
    pub order_type: OrderType,
    /// Limit price. Required for `Limit`, `Ioc`, `Fok`, `PostOnly`, `StopLimit`, `Oco`.
    pub price: Option<Decimal>,
    /// Total original quantity.
    pub quantity: Decimal,
    /// Remaining quantity to match or rest.
    pub remaining: Decimal,
    /// Self-trade prevention mode. Defaults to `Decrement`.
    pub stp_mode: StpMode,
    /// Trigger price for `StopLimit` / `StopMarket` orders.
    pub stop_price: Option<Decimal>,
    /// For `Oco` orders: the ID of the paired leg.
    pub oco_linked_id: Option<Uuid>,
    /// For `Iceberg` orders: maximum quantity to show on the book at once.
    pub display_qty: Option<Decimal>,
}

/// A single trade between a taker and a maker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    /// Taker order ID.
    pub taker_order_id: Uuid,
    /// Maker order ID.
    pub maker_order_id: Uuid,
    /// Execution price (always the maker's resting price).
    pub price: Decimal,
    /// Quantity traded.
    pub quantity: Decimal,
    /// Side of the taker.
    pub taker_side: Side,
}

/// Core domain errors.
#[derive(Debug, Error)]
pub enum Error {
    /// A stop order was submitted without a `stop_price`.
    #[error("stop order missing stop_price")]
    MissingStopPrice,

    /// An iceberg order was submitted without a `display_qty`.
    #[error("iceberg order missing display_qty")]
    MissingDisplayQty,

    /// A limit-type order was submitted without a `price`.
    #[error("limit-type order missing price")]
    MissingPrice,

    /// An OCO order was submitted without an `oco_linked_id`.
    #[error("oco order missing oco_linked_id")]
    MissingOcoLink,
}
