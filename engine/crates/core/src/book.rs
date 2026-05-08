use std::collections::{BTreeMap, VecDeque};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{Fill, Order, OrderType, Side};

/// Output event from the matching process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineEvent {
    /// A trade occurred.
    Fill(Fill),
    /// Order was rested on the book.
    Rested { order_id: Uuid, remaining: Decimal },
    /// Order was cancelled due to no liquidity (e.g. market order).
    Cancelled { order_id: Uuid, reason: String },
}

/// The order book for a single symbol.
#[derive(Debug, Clone, Default)]
pub struct OrderBook {
    /// Bids (Buy orders) - ordered descending by price.
    /// To iterate from best (highest) to worst (lowest), we use a custom key wrapper or `rev()`.
    /// Rust's BTreeMap is ascending. We'll store it as standard and `.iter().rev()` for bids.
    bids: BTreeMap<Decimal, VecDeque<Order>>,
    /// Asks (Sell orders) - ordered ascending by price.
    asks: BTreeMap<Decimal, VecDeque<Order>>,
}

impl OrderBook {
    /// Create a new empty order book.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an incoming taker order, generating a sequence of events.
    pub fn match_order(&mut self, mut taker: Order) -> Vec<EngineEvent> {
        let mut events = Vec::new();

        while taker.remaining > Decimal::ZERO {
            // Find the best price level on the opposite side.
            let best_price = self.best_opposite_price(taker.side);
            
            let Some(price) = best_price else {
                break;
            };

            // Check if price matches (if taker is Limit)
            if taker.order_type == OrderType::Limit {
                let taker_price = taker.price.expect("Limit order must have a price");
                match taker.side {
                    Side::Buy => {
                        // Buy limit order price must be >= best ask price
                        if taker_price < price {
                            break;
                        }
                    }
                    Side::Sell => {
                        // Sell limit order price must be <= best bid price
                        if taker_price > price {
                            break;
                        }
                    }
                }
            }

            // Execute against the level
            let level = match taker.side {
                Side::Buy => self.asks.get_mut(&price).expect("Price level exists"),
                Side::Sell => self.bids.get_mut(&price).expect("Price level exists"),
            };

            let maker = level.front_mut().expect("Level must have at least one order");
            
            // TODO: STP logic goes here (Phase 2)

            let match_qty = taker.remaining.min(maker.remaining);

            // Record fill
            events.push(EngineEvent::Fill(Fill {
                taker_order_id: taker.id,
                maker_order_id: maker.id,
                price,
                quantity: match_qty,
                taker_side: taker.side,
            }));

            taker.remaining -= match_qty;
            maker.remaining -= match_qty;

            // Remove maker if fully filled
            if maker.remaining.is_zero() {
                level.pop_front();
                if level.is_empty() {
                    self.remove_level(taker.side.opposite(), price);
                }
            }
        }

        if taker.remaining > Decimal::ZERO {
            if taker.order_type == OrderType::Market {
                events.push(EngineEvent::Cancelled {
                    order_id: taker.id,
                    reason: "no_liquidity".to_string(),
                });
            } else {
                // Rest limit order
                events.push(EngineEvent::Rested {
                    order_id: taker.id,
                    remaining: taker.remaining,
                });
                self.insert_maker(taker);
            }
        }

        events
    }

    /// Insert a maker order into the book.
    fn insert_maker(&mut self, maker: Order) {
        let price = maker.price.expect("Maker order must have a price");
        let level = match maker.side {
            Side::Buy => self.bids.entry(price).or_default(),
            Side::Sell => self.asks.entry(price).or_default(),
        };
        level.push_back(maker);
    }

    /// Get the best price on the given side.
    fn best_opposite_price(&self, taker_side: Side) -> Option<Decimal> {
        match taker_side {
            // Taker is Buy, look at Asks (lowest first)
            Side::Buy => self.asks.keys().next().copied(),
            // Taker is Sell, look at Bids (highest first)
            Side::Sell => self.bids.keys().next_back().copied(),
        }
    }

    /// Remove an empty price level.
    fn remove_level(&mut self, side: Side, price: Decimal) {
        match side {
            Side::Buy => self.bids.remove(&price),
            Side::Sell => self.asks.remove(&price),
        };
    }

    /// Get the current best bid.
    #[must_use]
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next_back().copied()
    }

    /// Get the current best ask.
    #[must_use]
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().copied()
    }

    /// Calculate total remaining quantity on the book.
    #[must_use]
    pub fn total_depth(&self) -> (Decimal, Decimal) {
        let bid_depth: Decimal = self
            .bids
            .values()
            .flat_map(|q| q.iter())
            .map(|o| o.remaining)
            .sum();
        let ask_depth: Decimal = self
            .asks
            .values()
            .flat_map(|q| q.iter())
            .map(|o| o.remaining)
            .sum();
        (bid_depth, ask_depth)
    }
}
