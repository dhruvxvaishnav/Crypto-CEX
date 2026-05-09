use std::collections::{BTreeMap, HashMap, VecDeque};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::stop::StopRegistry;
use crate::types::{CancelReason, Fill, Order, OrderType, Side, StpMode};

/// Maximum stop-trigger chain depth to prevent runaway cascades.
const MAX_STOP_CHAIN_DEPTH: u8 = 16;

/// Output event from the matching process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineEvent {
    /// A trade occurred between a taker and a maker.
    Fill(Fill),
    /// An order rested on the book after partial or zero matching.
    Rested { order_id: Uuid, remaining: Decimal },
    /// An order was cancelled (no state mutation occurred for FOK/post-only pre-rejections).
    Cancelled {
        order_id: Uuid,
        reason: CancelReason,
    },
    /// A stop order was triggered and converted to an active order.
    StopTriggered { order_id: Uuid },
}

/// Snapshot of a maker at the front of a price level, gathered via immutable borrow.
struct PeekedMaker {
    id: Uuid,
    user_id: Uuid,
    /// Effective matchable quantity: `min(display_qty, remaining)` for icebergs, else `remaining`.
    effective_qty: Decimal,
    remaining: Decimal,
    has_display_qty: bool,
}

/// Outcome returned by STP handlers indicating whether the taker is consumed.
enum StpOutcome {
    /// Taker was cancelled or exhausted; exit the matching loop.
    TakerConsumed,
    /// Taker survives; continue matching against the next level.
    Continue,
}

/// The order book for a single symbol.
///
/// Maintains price-time priority (PRD §14.2): bids match highest-first; asks match
/// lowest-first; FIFO within each price level.
///
/// `Clone` is implemented manually: `order_index` is left empty in the clone and must
/// be rebuilt with [`rebuild_index`](Self::rebuild_index) before `cancel_order` is used.
/// This matches the snapshot-load contract and keeps clone O(n) in order data only.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OrderBook {
    /// Buy orders (bids), keyed ascending; iterate with `.rev()` for best-first.
    bids: BTreeMap<Decimal, VecDeque<Order>>,
    /// Sell orders (asks), keyed ascending; iterate normally for best-first.
    asks: BTreeMap<Decimal, VecDeque<Order>>,
    /// Fast O(1) lookup for cancellation: `order_id` → (side, price).
    /// Rebuilt from `bids`/`asks` after snapshot load; not serialised.
    #[serde(skip)]
    order_index: HashMap<Uuid, (Side, Decimal)>,
    /// Inactive stop orders waiting for a trigger price.
    stops: StopRegistry,
    /// OCO pair registry: `order_id` → `linked_order_id`.
    oco_links: HashMap<Uuid, Uuid>,
}

impl Clone for OrderBook {
    /// Clones the book's order data and stop/OCO state.
    ///
    /// `order_index` is intentionally left empty; call [`rebuild_index`](Self::rebuild_index)
    /// if `cancel_order` is needed on the clone.
    fn clone(&self) -> Self {
        Self {
            bids: self.bids.clone(),
            asks: self.asks.clone(),
            order_index: HashMap::new(),
            stops: self.stops.clone(),
            oco_links: self.oco_links.clone(),
        }
    }
}

// ── Public API ─────────────────────────────────────────────────────────────────

impl OrderBook {
    /// Creates a new empty order book.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes an incoming order, returning the sequence of resulting events.
    ///
    /// Handles all order types: Limit, Market, IOC, FOK, `PostOnly`, `StopLimit`,
    /// `StopMarket`, OCO, and Iceberg (PRD §14.3–14.5).
    pub fn match_order(&mut self, taker: Order) -> Vec<EngineEvent> {
        // PRD §14.4: FOK and post-only are pre-checked before any mutation.
        if let Some(reject) = self.pre_check(&taker) {
            return vec![reject];
        }

        // Stop orders live in the registry until triggered (PRD §14.5).
        if taker.order_type.is_stop() {
            return self.register_stop_order(taker);
        }

        // Core matching with stop-trigger cascade.
        let mut all_events = Vec::new();
        let mut work_queue = vec![taker];
        let mut stop_depth: u8 = 0;

        while let Some(order) = work_queue.pop() {
            let (events, triggered) = self.run_matching_loop(order);
            all_events.extend(events);
            if stop_depth < MAX_STOP_CHAIN_DEPTH {
                stop_depth =
                    stop_depth.saturating_add(u8::try_from(triggered.len()).unwrap_or(u8::MAX));
                for t in &triggered {
                    all_events.push(EngineEvent::StopTriggered { order_id: t.id });
                }
                work_queue.extend(triggered);
            }
        }

        all_events
    }

    /// Cancels a resting or stop order by ID.
    ///
    /// Returns events emitted (the cancel event, plus an OCO-linked cancel if applicable).
    /// Returns an empty `Vec` if the order is not found.
    pub fn cancel_order(&mut self, order_id: Uuid) -> Vec<EngineEvent> {
        let linked = self.oco_links.remove(&order_id);
        if let Some(linked_id) = linked {
            self.oco_links.remove(&linked_id);
        }

        let mut events = self.cancel_order_internal(order_id, CancelReason::AdminCancel);

        if let Some(linked_id) = linked {
            events.extend(self.cancel_order_internal(linked_id, CancelReason::Oco));
        }

        events
    }

    /// Registers an OCO pair so that when one leg becomes terminal the other is cancelled.
    ///
    /// **MUST** be called before submitting either leg via [`match_order`](Self::match_order).
    pub fn register_oco(&mut self, leg_a: Uuid, leg_b: Uuid) {
        self.oco_links.insert(leg_a, leg_b);
        self.oco_links.insert(leg_b, leg_a);
    }

    /// Returns the current best bid price, if any.
    #[must_use]
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next_back().copied()
    }

    /// Returns the current best ask price, if any.
    #[must_use]
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().copied()
    }

    /// Returns `(total_bid_qty, total_ask_qty)` — sum of all resting quantities.
    #[must_use]
    pub fn total_depth(&self) -> (Decimal, Decimal) {
        let bid = self
            .bids
            .values()
            .flat_map(VecDeque::iter)
            .map(|o| o.remaining)
            .sum();
        let ask = self
            .asks
            .values()
            .flat_map(VecDeque::iter)
            .map(|o| o.remaining)
            .sum();
        (bid, ask)
    }

    /// Rebuilds the `order_index` from the resting `bids` and `asks`.
    ///
    /// Call this after loading an [`OrderBook`] from a snapshot, since `order_index`
    /// is not serialised (it is derived from the book state).
    pub fn rebuild_index(&mut self) {
        self.order_index.clear();
        for (price, queue) in &self.bids {
            for order in queue {
                self.order_index.insert(order.id, (Side::Buy, *price));
            }
        }
        for (price, queue) in &self.asks {
            for order in queue {
                self.order_index.insert(order.id, (Side::Sell, *price));
            }
        }
    }
}

// ── Pre-matching helpers ────────────────────────────────────────────────────────

impl OrderBook {
    /// Performs stateless pre-checks for FOK and post-only orders (PRD §14.4).
    ///
    /// Returns a `Cancelled` event if the order must be rejected without state change,
    /// or `None` if matching may proceed.
    fn pre_check(&self, taker: &Order) -> Option<EngineEvent> {
        match taker.order_type {
            OrderType::Fok if !self.can_fully_fill(taker) => Some(EngineEvent::Cancelled {
                order_id: taker.id,
                reason: CancelReason::FokUnfilled,
            }),
            OrderType::PostOnly if self.would_cross_post_only(taker) => {
                Some(EngineEvent::Cancelled {
                    order_id: taker.id,
                    reason: CancelReason::PostOnlyCrossed,
                })
            }
            _ => None,
        }
    }

    /// Walks the book to determine if `taker` quantity is available at acceptable prices.
    ///
    /// For limit FOK, only counts quantity at prices ≤ taker's price (buy) or ≥ (sell).
    /// For market FOK, all available quantity counts.
    fn can_fully_fill(&self, taker: &Order) -> bool {
        let mut available = Decimal::ZERO;
        match taker.side {
            Side::Buy => {
                for (price, queue) in &self.asks {
                    if taker.price.map_or(false, |limit| *price > limit) {
                        break;
                    }
                    available += queue.iter().map(|o| o.remaining).sum::<Decimal>();
                    if available >= taker.remaining {
                        return true;
                    }
                }
            }
            Side::Sell => {
                for (price, queue) in self.bids.iter().rev() {
                    if taker.price.map_or(false, |limit| *price < limit) {
                        break;
                    }
                    available += queue.iter().map(|o| o.remaining).sum::<Decimal>();
                    if available >= taker.remaining {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns `true` if a post-only taker would cross the book on entry (PRD §14.4).
    fn would_cross_post_only(&self, taker: &Order) -> bool {
        let Some(taker_price) = taker.price else {
            return false;
        };
        match taker.side {
            Side::Buy => self.best_ask().map_or(false, |best| best <= taker_price),
            Side::Sell => self.best_bid().map_or(false, |best| best >= taker_price),
        }
    }

    /// Returns `true` if `price` satisfies the taker's limit constraint.
    fn taker_price_satisfied(taker: &Order, price: Decimal) -> bool {
        let Some(limit) = taker.price else {
            return true;
        }; // market orders always match
        match taker.side {
            Side::Buy => price <= limit,
            Side::Sell => price >= limit,
        }
    }
}

// ── Stop helpers ────────────────────────────────────────────────────────────────

impl OrderBook {
    /// Registers a stop order in the stop registry and emits a `Rested` event.
    fn register_stop_order(&mut self, order: Order) -> Vec<EngineEvent> {
        let order_id = order.id;
        let remaining = order.remaining;
        match self.stops.insert(order) {
            Ok(()) => vec![EngineEvent::Rested {
                order_id,
                remaining,
            }],
            Err(_) => vec![EngineEvent::Cancelled {
                order_id,
                reason: CancelReason::AdminCancel,
            }],
        }
    }

    /// Drains stop orders triggered by `last_trade_price`, converting them to active orders.
    fn check_stop_triggers(&mut self, last_price: Decimal) -> Vec<Order> {
        self.stops.drain_triggered(last_price)
    }
}

// ── Core matching loop ──────────────────────────────────────────────────────────

impl OrderBook {
    /// Executes the matching algorithm and returns `(events, triggered_stop_orders)`.
    fn run_matching_loop(&mut self, mut taker: Order) -> (Vec<EngineEvent>, Vec<Order>) {
        let mut events: Vec<EngineEvent> = Vec::new();
        let mut last_trade_price: Option<Decimal> = None;
        let mut had_fills = false;

        while taker.remaining > Decimal::ZERO {
            let Some(price) = self.best_opposite_price(taker.side) else {
                break;
            };
            if !Self::taker_price_satisfied(&taker, price) {
                break;
            }

            let maker_side = taker.side.opposite();
            let Some(peeked) = self.peek_front_maker(maker_side, price) else {
                self.remove_level(maker_side, price);
                continue;
            };

            if taker.user_id == peeked.user_id {
                match self.apply_stp(&mut taker, price, &peeked, &mut events) {
                    StpOutcome::TakerConsumed => {
                        let triggered =
                            last_trade_price.map_or_else(Vec::new, |p| self.check_stop_triggers(p));
                        return (events, triggered);
                    }
                    StpOutcome::Continue => continue,
                }
            }

            let fill_qty = taker.remaining.min(peeked.effective_qty);
            events.push(EngineEvent::Fill(Fill {
                taker_order_id: taker.id,
                maker_order_id: peeked.id,
                price,
                quantity: fill_qty,
                taker_side: taker.side,
            }));
            taker.remaining -= fill_qty;
            had_fills = true;
            last_trade_price = Some(price);

            let maker_fully_filled = self.apply_fill_to_maker(maker_side, price, fill_qty, &peeked);
            if maker_fully_filled {
                events.extend(self.maybe_cancel_oco(peeked.id));
            }
        }

        let finalize_events = self.finalize_taker(&taker, had_fills);
        // Cancel the OCO link only when this taker becomes terminal (filled or cancelled).
        // If taker rested on the book, the OCO link must remain active.
        let taker_terminal = taker.remaining.is_zero()
            || finalize_events
                .iter()
                .any(|e| matches!(e, EngineEvent::Cancelled { .. }));
        events.extend(finalize_events);
        if taker_terminal {
            events.extend(self.maybe_cancel_oco(taker.id));
        }

        let triggered = last_trade_price.map_or_else(Vec::new, |p| self.check_stop_triggers(p));
        (events, triggered)
    }

    /// Returns an immutable snapshot of the front maker at a price level.
    fn peek_front_maker(&self, maker_side: Side, price: Decimal) -> Option<PeekedMaker> {
        let level = match maker_side {
            Side::Buy => self.bids.get(&price)?,
            Side::Sell => self.asks.get(&price)?,
        };
        let maker = level.front()?;
        let effective_qty = maker
            .display_qty
            .map_or(maker.remaining, |d| d.min(maker.remaining));
        Some(PeekedMaker {
            id: maker.id,
            user_id: maker.user_id,
            effective_qty,
            remaining: maker.remaining,
            has_display_qty: maker.display_qty.is_some(),
        })
    }

    /// Applies a fill to the front maker, handling iceberg re-queue.
    ///
    /// Returns `true` if the maker was fully consumed.
    fn apply_fill_to_maker(
        &mut self,
        maker_side: Side,
        price: Decimal,
        fill_qty: Decimal,
        peeked: &PeekedMaker,
    ) -> bool {
        let peak_consumed = peeked.has_display_qty && fill_qty == peeked.effective_qty;

        let (maker_done, level_empty, maker_id) = {
            let level = match maker_side {
                Side::Buy => self.bids.get_mut(&price),
                Side::Sell => self.asks.get_mut(&price),
            };
            let Some(level) = level else { return false };
            let Some(maker) = level.front_mut() else {
                return false;
            };

            maker.remaining -= fill_qty;
            let done = maker.remaining.is_zero();
            let id = maker.id;

            if done {
                level.pop_front();
            } else if peak_consumed {
                // Re-queue at back to refresh the iceberg display slice.
                // Loses time priority within the level (PRD §1.3 case 7).
                if let Some(o) = level.pop_front() {
                    level.push_back(o);
                }
            }

            (done, level.is_empty(), id)
        };

        if maker_done {
            self.order_index.remove(&maker_id);
        }
        if level_empty {
            self.remove_level(maker_side, price);
        }

        maker_done
    }
}

// ── STP helpers ─────────────────────────────────────────────────────────────────

impl OrderBook {
    /// Dispatches to the appropriate STP handler based on the taker's STP mode (PRD §14.3).
    fn apply_stp(
        &mut self,
        taker: &mut Order,
        price: Decimal,
        peeked: &PeekedMaker,
        events: &mut Vec<EngineEvent>,
    ) -> StpOutcome {
        match taker.stp_mode {
            StpMode::CancelTaker => {
                events.push(EngineEvent::Cancelled {
                    order_id: taker.id,
                    reason: CancelReason::SelfTradePrevented,
                });
                StpOutcome::TakerConsumed
            }
            StpMode::CancelMaker => {
                events.push(EngineEvent::Cancelled {
                    order_id: peeked.id,
                    reason: CancelReason::SelfTradePrevented,
                });
                self.pop_front_maker(taker.side.opposite(), price, peeked.id);
                StpOutcome::Continue
            }
            StpMode::Decrement => self.apply_stp_decrement(taker, price, peeked, events),
        }
    }

    /// STP Decrement: cancel the smaller side; if equal, cancel both (PRD FR-TRADE-06).
    fn apply_stp_decrement(
        &mut self,
        taker: &mut Order,
        price: Decimal,
        peeked: &PeekedMaker,
        events: &mut Vec<EngineEvent>,
    ) -> StpOutcome {
        let maker_side = taker.side.opposite();
        match taker.remaining.cmp(&peeked.remaining) {
            std::cmp::Ordering::Less => {
                // Taker is smaller → cancel taker, decrement maker.
                events.push(EngineEvent::Cancelled {
                    order_id: taker.id,
                    reason: CancelReason::SelfTradePrevented,
                });
                self.decrement_front_maker(maker_side, price, taker.remaining);
                taker.remaining = Decimal::ZERO;
                StpOutcome::TakerConsumed
            }
            std::cmp::Ordering::Greater => {
                // Maker is smaller → cancel maker, taker continues.
                events.push(EngineEvent::Cancelled {
                    order_id: peeked.id,
                    reason: CancelReason::SelfTradePrevented,
                });
                taker.remaining -= peeked.remaining;
                self.pop_front_maker(maker_side, price, peeked.id);
                StpOutcome::Continue
            }
            std::cmp::Ordering::Equal => {
                // Equal → cancel both.
                events.push(EngineEvent::Cancelled {
                    order_id: peeked.id,
                    reason: CancelReason::SelfTradePrevented,
                });
                events.push(EngineEvent::Cancelled {
                    order_id: taker.id,
                    reason: CancelReason::SelfTradePrevented,
                });
                taker.remaining = Decimal::ZERO;
                self.pop_front_maker(maker_side, price, peeked.id);
                StpOutcome::TakerConsumed
            }
        }
    }

    /// Pops the front maker off a price level, updating the index.
    fn pop_front_maker(&mut self, maker_side: Side, price: Decimal, maker_id: Uuid) {
        let level_empty = {
            let level = match maker_side {
                Side::Buy => self.bids.get_mut(&price),
                Side::Sell => self.asks.get_mut(&price),
            };
            let Some(level) = level else { return };
            level.pop_front();
            level.is_empty()
        };
        self.order_index.remove(&maker_id);
        if level_empty {
            self.remove_level(maker_side, price);
        }
    }

    /// Decrements the remaining quantity of the front maker (Decrement STP — maker survives).
    fn decrement_front_maker(&mut self, maker_side: Side, price: Decimal, by: Decimal) {
        let (maker_done, maker_id) = {
            let level = match maker_side {
                Side::Buy => self.bids.get_mut(&price),
                Side::Sell => self.asks.get_mut(&price),
            };
            let Some(level) = level else { return };
            let Some(maker) = level.front_mut() else {
                return;
            };
            maker.remaining -= by;
            (maker.remaining.is_zero(), maker.id)
        };
        if maker_done {
            self.pop_front_maker(maker_side, price, maker_id);
        }
    }
}

// ── Finalization & OCO ──────────────────────────────────────────────────────────

impl OrderBook {
    /// Determines what to emit for a taker whose matching loop has ended.
    fn finalize_taker(&mut self, taker: &Order, had_fills: bool) -> Vec<EngineEvent> {
        if taker.remaining.is_zero() {
            return Vec::new(); // fully matched — caller handles OCO
        }
        match taker.order_type {
            OrderType::Market | OrderType::Ioc => vec![EngineEvent::Cancelled {
                order_id: taker.id,
                reason: CancelReason::NoLiquidity,
            }],
            // FOK: remaining > 0 after pre-check passed means partial fill occurred.
            // Pre-check prevents this; emit cancel defensively rather than panicking.
            OrderType::Fok => vec![EngineEvent::Cancelled {
                order_id: taker.id,
                reason: CancelReason::FokUnfilled,
            }],
            // PostOnly with had_fills is logically impossible (pre_check prevents it).
            OrderType::PostOnly if had_fills => vec![EngineEvent::Cancelled {
                order_id: taker.id,
                reason: CancelReason::PostOnlyCrossed,
            }],
            OrderType::Limit | OrderType::PostOnly | OrderType::Iceberg | OrderType::Oco => {
                self.insert_maker(taker.clone());
                vec![EngineEvent::Rested {
                    order_id: taker.id,
                    remaining: taker.remaining,
                }]
            }
            // Stops are handled before run_matching_loop; this branch is unreachable.
            OrderType::StopLimit | OrderType::StopMarket => vec![EngineEvent::Cancelled {
                order_id: taker.id,
                reason: CancelReason::AdminCancel,
            }],
        }
    }

    /// If `order_id` has an OCO link, cancels the linked leg and returns its events.
    fn maybe_cancel_oco(&mut self, order_id: Uuid) -> Vec<EngineEvent> {
        let Some(linked_id) = self.oco_links.remove(&order_id) else {
            return Vec::new();
        };
        self.oco_links.remove(&linked_id);
        self.cancel_order_internal(linked_id, CancelReason::Oco)
    }

    /// Cancels an order without propagating to OCO links (used internally to avoid cycles).
    fn cancel_order_internal(&mut self, order_id: Uuid, reason: CancelReason) -> Vec<EngineEvent> {
        if self.stops.cancel(order_id) {
            return vec![EngineEvent::Cancelled { order_id, reason }];
        }

        let Some((side, price)) = self.order_index.remove(&order_id) else {
            return Vec::new();
        };

        let level_empty = {
            let level = match side {
                Side::Buy => self.bids.get_mut(&price),
                Side::Sell => self.asks.get_mut(&price),
            };
            let Some(level) = level else {
                return Vec::new();
            };
            if let Some(pos) = level.iter().position(|o| o.id == order_id) {
                level.remove(pos);
            }
            level.is_empty()
        };

        if level_empty {
            self.remove_level(side, price);
        }

        vec![EngineEvent::Cancelled { order_id, reason }]
    }
}

// ── Book maintenance ────────────────────────────────────────────────────────────

impl OrderBook {
    /// Inserts a maker order into the appropriate side of the book.
    fn insert_maker(&mut self, order: Order) {
        let Some(price) = order.price else { return };
        self.order_index.insert(order.id, (order.side, price));
        let level = match order.side {
            Side::Buy => self.bids.entry(price).or_default(),
            Side::Sell => self.asks.entry(price).or_default(),
        };
        level.push_back(order);
    }

    /// Returns the best price on the side opposing `taker_side`.
    fn best_opposite_price(&self, taker_side: Side) -> Option<Decimal> {
        match taker_side {
            Side::Buy => self.asks.keys().next().copied(),
            Side::Sell => self.bids.keys().next_back().copied(),
        }
    }

    /// Removes a price level from the book.
    fn remove_level(&mut self, side: Side, price: Decimal) {
        match side {
            Side::Buy => self.bids.remove(&price),
            Side::Sell => self.asks.remove(&price),
        };
    }
}
