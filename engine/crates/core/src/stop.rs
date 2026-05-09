use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{Error, Order, OrderType, Side};

/// Registry of stop orders waiting to be triggered by trade prices.
#[allow(clippy::module_name_repetitions)]
///
/// Per PRD §14.5:
/// - Buy stops trigger when `last_trade_price >= stop_price`.
/// - Sell stops trigger when `last_trade_price <= stop_price`.
///
/// After triggering, `StopLimit` converts to `Limit` and `StopMarket` converts to `Market`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StopRegistry {
    /// Buy-side stops keyed by `stop_price` ascending.
    /// Drain keys where `stop_price <= last_trade_price`.
    buy_stops: BTreeMap<Decimal, Vec<Order>>,
    /// Sell-side stops keyed by `stop_price` descending.
    /// We use a `BTreeMap` (ascending) and drain in reverse for sells.
    sell_stops: BTreeMap<Decimal, Vec<Order>>,
}

impl StopRegistry {
    /// Creates an empty stop registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a stop order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingStopPrice`] if `order.stop_price` is `None`.
    pub fn insert(&mut self, order: Order) -> Result<(), Error> {
        let stop_price = order.stop_price.ok_or(Error::MissingStopPrice)?;
        let bucket = match order.side {
            Side::Buy => self.buy_stops.entry(stop_price).or_default(),
            Side::Sell => self.sell_stops.entry(stop_price).or_default(),
        };
        bucket.push(order);
        Ok(())
    }

    /// Cancels a stop order by ID. Returns `true` if found and removed.
    pub fn cancel(&mut self, order_id: Uuid) -> bool {
        for bucket in self.buy_stops.values_mut() {
            if let Some(pos) = bucket.iter().position(|o| o.id == order_id) {
                bucket.swap_remove(pos);
                return true;
            }
        }
        for bucket in self.sell_stops.values_mut() {
            if let Some(pos) = bucket.iter().position(|o| o.id == order_id) {
                bucket.swap_remove(pos);
                return true;
            }
        }
        false
    }

    /// Drains all stop orders triggered by `last_trade_price`, converting them to
    /// active `Limit` or `Market` orders ready to re-enter the matching loop.
    ///
    /// Returns the converted orders in insertion order within each price level.
    pub fn drain_triggered(&mut self, last_trade_price: Decimal) -> Vec<Order> {
        let mut triggered = Vec::new();
        triggered.extend(self.drain_buy_stops(last_trade_price));
        triggered.extend(self.drain_sell_stops(last_trade_price));
        triggered
    }

    /// Returns the total number of stop orders in the registry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buy_stops.values().map(Vec::len).sum::<usize>()
            + self.sell_stops.values().map(Vec::len).sum::<usize>()
    }

    /// Returns `true` if the registry contains no stop orders.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drains buy stops where `stop_price <= last_trade_price`.
    fn drain_buy_stops(&mut self, last_trade_price: Decimal) -> Vec<Order> {
        // BTreeMap is ascending: take all keys ≤ last_trade_price.
        let trigger_keys: Vec<Decimal> = self
            .buy_stops
            .range(..=last_trade_price)
            .map(|(k, _)| *k)
            .collect();

        let mut out = Vec::new();
        for key in trigger_keys {
            if let Some(orders) = self.buy_stops.remove(&key) {
                out.extend(orders.into_iter().map(convert_stop));
            }
        }
        out
    }

    /// Drains sell stops where `stop_price >= last_trade_price`.
    fn drain_sell_stops(&mut self, last_trade_price: Decimal) -> Vec<Order> {
        // BTreeMap is ascending: take all keys ≥ last_trade_price.
        let trigger_keys: Vec<Decimal> = self
            .sell_stops
            .range(last_trade_price..)
            .map(|(k, _)| *k)
            .collect();

        let mut out = Vec::new();
        for key in trigger_keys {
            if let Some(orders) = self.sell_stops.remove(&key) {
                out.extend(orders.into_iter().map(convert_stop));
            }
        }
        out
    }
}

/// Converts a stop order to its active form (`StopLimit` → `Limit`, `StopMarket` → `Market`).
#[allow(clippy::missing_const_for_fn)] // Order is not Copy; match on non-Copy fields can't be const
fn convert_stop(mut order: Order) -> Order {
    order.order_type = match order.order_type {
        OrderType::StopLimit => OrderType::Limit,
        OrderType::StopMarket => OrderType::Market,
        other => other,
    };
    order
}
