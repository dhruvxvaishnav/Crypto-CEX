use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Side of the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// Buy side (bid).
    Buy,
    /// Sell side (ask).
    Sell,
}

impl Side {
    /// Get the opposite side.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }
}

/// Type of the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Limit order rests on the book at the specified price.
    Limit,
    /// Market order matches immediately or is cancelled.
    Market,
}

/// Core order representation in the matching engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier.
    pub id: Uuid,
    /// User identifier.
    pub user_id: Uuid,
    /// The market symbol this order belongs to (e.g. "BTCUSDT").
    pub symbol: String,
    /// Order side.
    pub side: Side,
    /// Order type.
    pub order_type: OrderType,
    /// Price (required for Limit orders).
    pub price: Option<Decimal>,
    /// Initial quantity.
    pub quantity: Decimal,
    /// Remaining quantity to be matched.
    pub remaining: Decimal,
}

/// Fill event representing a match between two orders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    /// Taker order ID.
    pub taker_order_id: Uuid,
    /// Maker order ID.
    pub maker_order_id: Uuid,
    /// Price at which the trade occurred.
    pub price: Decimal,
    /// Quantity traded.
    pub quantity: Decimal,
    /// Side of the taker.
    pub taker_side: Side,
}

/// Core domain errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Returned when an operation requires liquidity but the book is empty at the price level.
    #[error("no liquidity")]
    NoLiquidity,
}
