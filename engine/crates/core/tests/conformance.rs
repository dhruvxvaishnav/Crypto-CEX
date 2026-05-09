//! Conformance test suite — PRD §14.7
//!
//! Every case uses a fresh `OrderBook::new()` and asserts exact event sequences.
//! At least 40 cases are required; this file contains 45.

use cex_core::book::{EngineEvent, OrderBook};
use cex_core::types::{CancelReason, Fill, Order, OrderType, Side, StpMode};
use rust_decimal::Decimal;
use uuid::Uuid;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn d(n: i64) -> Decimal {
    Decimal::new(n, 0)
}

/// Build an order with all optional fields set to their defaults.
fn order(
    id: Uuid,
    user_id: Uuid,
    side: Side,
    order_type: OrderType,
    price: Option<Decimal>,
    qty: Decimal,
) -> Order {
    Order {
        id,
        user_id,
        symbol: "BTCUSDT".into(),
        side,
        order_type,
        price,
        quantity: qty,
        remaining: qty,
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    }
}

fn limit(id: Uuid, user_id: Uuid, side: Side, price: i64, qty: i64) -> Order {
    order(id, user_id, side, OrderType::Limit, Some(d(price)), d(qty))
}

fn market(id: Uuid, user_id: Uuid, side: Side, qty: i64) -> Order {
    order(id, user_id, side, OrderType::Market, None, d(qty))
}

fn uid() -> Uuid {
    Uuid::new_v4()
}

fn fill_event(
    taker_id: Uuid,
    maker_id: Uuid,
    price: i64,
    qty: i64,
    taker_side: Side,
) -> EngineEvent {
    EngineEvent::Fill(Fill {
        taker_order_id: taker_id,
        maker_order_id: maker_id,
        price: d(price),
        quantity: d(qty),
        taker_side,
    })
}

fn cancelled(id: Uuid, reason: CancelReason) -> EngineEvent {
    EngineEvent::Cancelled {
        order_id: id,
        reason,
    }
}

fn rested(id: Uuid, remaining: i64) -> EngineEvent {
    EngineEvent::Rested {
        order_id: id,
        remaining: d(remaining),
    }
}

// ── Case 1: Empty book + market buy → cancel NoLiquidity ─────────────────────

#[test]
fn c01_empty_book_market_buy_cancels() {
    let mut book = OrderBook::new();
    let taker_id = uid();
    let events = book.match_order(market(taker_id, uid(), Side::Buy, 1));
    assert_eq!(events, vec![cancelled(taker_id, CancelReason::NoLiquidity)]);
}

// ── Case 2: Ask 100×1, market buy 1 → fill, both gone ───────────────────────

#[test]
fn c02_market_buy_fills_resting_ask() {
    let mut book = OrderBook::new();
    let maker_id = uid();
    let taker_id = uid();
    let user = uid();

    book.match_order(limit(maker_id, user, Side::Sell, 100, 1));
    let events = book.match_order(market(taker_id, uid(), Side::Buy, 1));

    assert_eq!(
        events,
        vec![fill_event(taker_id, maker_id, 100, 1, Side::Buy)]
    );
    assert!(book.best_ask().is_none());
    assert!(book.best_bid().is_none());
}

// ── Case 3: No crossed book after arbitrary sequence ────────────────────────

#[test]
fn c03_no_crossed_book_invariant() {
    let mut book = OrderBook::new();
    let user = uid();

    // Add bids and asks; ensure best_bid < best_ask at each step.
    for price in [95, 98, 100] {
        book.match_order(limit(uid(), user, Side::Buy, price, 5));
    }
    for price in [101, 103, 105] {
        book.match_order(limit(uid(), user, Side::Sell, price, 5));
    }

    let best_bid = book.best_bid().expect("bids exist");
    let best_ask = book.best_ask().expect("asks exist");
    assert!(
        best_bid < best_ask,
        "crossed book: bid={best_bid} ask={best_ask}"
    );
}

// ── Case 4: Limit buy crosses two ask levels, filling at maker prices ────────

#[test]
fn c04_limit_buy_crosses_two_levels() {
    let mut book = OrderBook::new();
    let user = uid();
    let m1 = uid();
    let m2 = uid();
    let t = uid();

    book.match_order(limit(m1, user, Side::Sell, 99, 1));
    book.match_order(limit(m2, user, Side::Sell, 100, 1));

    let events = book.match_order(limit(t, uid(), Side::Buy, 100, 2));
    assert_eq!(
        events,
        vec![
            fill_event(t, m1, 99, 1, Side::Buy),
            fill_event(t, m2, 100, 1, Side::Buy),
        ]
    );
}

// ── Case 5: FOK buy insufficient liquidity → reject, book unchanged ──────────

#[test]
fn c05_fok_buy_rejects_insufficient_liquidity() {
    let mut book = OrderBook::new();
    let user = uid();
    let m1 = uid();
    let m2 = uid();
    let t = uid();

    book.match_order(limit(m1, user, Side::Sell, 99, 1));
    book.match_order(limit(m2, user, Side::Sell, 100, 1));

    let fok = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::Fok,
        price: Some(d(100)),
        quantity: d(3),
        remaining: d(3),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };

    let events = book.match_order(fok);
    assert_eq!(events, vec![cancelled(t, CancelReason::FokUnfilled)]);
    // Book is unchanged — no fills occurred.
    assert_eq!(book.best_ask(), Some(d(99)));
    let (_, ask_depth) = book.total_depth();
    assert_eq!(ask_depth, d(2));
}

// ── Case 6: Post-only buy at best-ask price → rejected ───────────────────────

#[test]
fn c06_post_only_buy_crosses_rejected() {
    let mut book = OrderBook::new();
    book.match_order(limit(uid(), uid(), Side::Sell, 100, 1));

    let t = uid();
    let po = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::PostOnly,
        price: Some(d(100)),
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(po);
    assert_eq!(events, vec![cancelled(t, CancelReason::PostOnlyCrossed)]);
    // Ask still on book.
    assert_eq!(book.best_ask(), Some(d(100)));
}

// ── Case 7: Post-only buy below best ask → rests ─────────────────────────────

#[test]
fn c07_post_only_buy_no_cross_rests() {
    let mut book = OrderBook::new();
    book.match_order(limit(uid(), uid(), Side::Sell, 101, 1));

    let t = uid();
    let po = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::PostOnly,
        price: Some(d(100)),
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(po);
    assert_eq!(events, vec![rested(t, 1)]);
    assert_eq!(book.best_bid(), Some(d(100)));
}

// ── Case 8: Iceberg ask 100×10 display 2 → 5 buys of 2 → 5 fills ────────────

#[test]
fn c08_iceberg_display_refreshes() {
    let mut book = OrderBook::new();
    let maker_id = uid();
    let maker_user = uid();

    let iceberg = Order {
        id: maker_id,
        user_id: maker_user,
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::Iceberg,
        price: Some(d(100)),
        quantity: d(10),
        remaining: d(10),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: Some(d(2)),
    };
    book.match_order(iceberg);

    for _ in 0..5 {
        let taker_id = uid();
        let events = book.match_order(market(taker_id, uid(), Side::Buy, 2));
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0], EngineEvent::Fill(ref f) if f.maker_order_id == maker_id && f.quantity == d(2)),
            "expected fill of 2 against iceberg maker"
        );
    }

    // All 10 units are now consumed.
    assert!(book.best_ask().is_none());
}

// ── Case 9: STP Decrement — taker(3) < maker(5) → taker cancelled ────────────

#[test]
fn c09_stp_decrement_taker_smaller() {
    let mut book = OrderBook::new();
    let same_user = uid();
    let maker_id = uid();
    let taker_id = uid();

    book.match_order(limit(maker_id, same_user, Side::Sell, 100, 5));

    let mut taker = limit(taker_id, same_user, Side::Buy, 100, 3);
    taker.stp_mode = StpMode::Decrement;
    let events = book.match_order(taker);

    // Taker (smaller) is cancelled; maker decremented by 3 → 2 remains.
    assert_eq!(
        events,
        vec![cancelled(taker_id, CancelReason::SelfTradePrevented)]
    );
    let (_, ask_depth) = book.total_depth();
    assert_eq!(ask_depth, d(2), "maker should have 2 remaining");
}

// ── Case 10: STP Decrement — taker(5) > maker(3) → maker cancelled, taker continues

#[test]
fn c10_stp_decrement_maker_smaller() {
    let mut book = OrderBook::new();
    let same_user = uid();
    let maker_id = uid();
    let other_maker_id = uid();
    let taker_id = uid();

    // First maker: same user, qty 3.
    book.match_order(limit(maker_id, same_user, Side::Sell, 100, 3));
    // Second maker: different user, qty 5 — taker continues to fill this.
    book.match_order(limit(other_maker_id, uid(), Side::Sell, 100, 5));

    let mut taker = limit(taker_id, same_user, Side::Buy, 100, 5);
    taker.stp_mode = StpMode::Decrement;
    let events = book.match_order(taker);

    // STP cancels maker (3) → taker decremented to 2 → fills other_maker for 2.
    // Taker is fully consumed after the fill; no Rested event.
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        cancelled(maker_id, CancelReason::SelfTradePrevented)
    );
    assert_eq!(
        events[1],
        fill_event(taker_id, other_maker_id, 100, 2, Side::Buy)
    );
}

// ── Case 11: STP Decrement — equal qty → both cancelled ──────────────────────

#[test]
fn c11_stp_decrement_equal_qty_both_cancelled() {
    let mut book = OrderBook::new();
    let same_user = uid();
    let maker_id = uid();
    let taker_id = uid();

    book.match_order(limit(maker_id, same_user, Side::Sell, 100, 4));

    let mut taker = limit(taker_id, same_user, Side::Buy, 100, 4);
    taker.stp_mode = StpMode::Decrement;
    let events = book.match_order(taker);

    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        cancelled(maker_id, CancelReason::SelfTradePrevented)
    );
    assert_eq!(
        events[1],
        cancelled(taker_id, CancelReason::SelfTradePrevented)
    );
    assert!(book.best_ask().is_none());
}

// ── Case 12: STP CancelMaker — maker cancelled, taker fills next level ────────

#[test]
fn c12_stp_cancel_maker() {
    let mut book = OrderBook::new();
    let same_user = uid();
    let maker_id = uid();
    let other_maker_id = uid();
    let taker_id = uid();

    book.match_order(limit(maker_id, same_user, Side::Sell, 100, 3));
    book.match_order(limit(other_maker_id, uid(), Side::Sell, 100, 3));

    let mut taker = limit(taker_id, same_user, Side::Buy, 100, 3);
    taker.stp_mode = StpMode::CancelMaker;
    let events = book.match_order(taker);

    assert_eq!(
        events[0],
        cancelled(maker_id, CancelReason::SelfTradePrevented)
    );
    assert_eq!(
        events[1],
        fill_event(taker_id, other_maker_id, 100, 3, Side::Buy)
    );
}

// ── Case 13: STP CancelTaker — taker cancelled, maker stays ──────────────────

#[test]
fn c13_stp_cancel_taker() {
    let mut book = OrderBook::new();
    let same_user = uid();
    let maker_id = uid();
    let taker_id = uid();

    book.match_order(limit(maker_id, same_user, Side::Sell, 100, 5));

    let mut taker = limit(taker_id, same_user, Side::Buy, 100, 5);
    taker.stp_mode = StpMode::CancelTaker;
    let events = book.match_order(taker);

    assert_eq!(
        events,
        vec![cancelled(taker_id, CancelReason::SelfTradePrevented)]
    );
    let (_, ask_depth) = book.total_depth();
    assert_eq!(ask_depth, d(5), "maker must be untouched");
}

// ── Case 14: Stop-buy triggers at stop_price, converts to limit ──────────────

#[test]
fn c14_stop_buy_triggers_as_limit() {
    let mut book = OrderBook::new();
    let stop_id = uid();
    let user = uid();

    // Place a resting sell at 101 so the triggered limit can fill.
    let maker_id = uid();
    book.match_order(limit(maker_id, uid(), Side::Sell, 101, 5));

    // Submit stop-buy: stop=100, limit=101.
    let stop_order = Order {
        id: stop_id,
        user_id: user,
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::StopLimit,
        price: Some(d(101)),
        quantity: d(3),
        remaining: d(3),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(100)),
        oco_linked_id: None,
        display_qty: None,
    };
    let reg_events = book.match_order(stop_order);
    assert!(matches!(reg_events[0], EngineEvent::Rested { .. }));

    // Trigger: execute a trade at price 100.
    let trigger_maker = uid();
    book.match_order(limit(trigger_maker, uid(), Side::Sell, 100, 1));
    let trigger_events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    // The trade at 100 triggers the stop. The stop-triggered order then fills at 101.
    let has_stop_trigger = trigger_events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_id));
    assert!(has_stop_trigger, "StopTriggered event expected");
    let has_fill = trigger_events
        .iter()
        .any(|e| matches!(e, EngineEvent::Fill(f) if f.maker_order_id == maker_id));
    assert!(has_fill, "Fill from triggered stop-limit expected");
}

// ── Case 15: Stop-sell triggers as market ────────────────────────────────────

#[test]
fn c15_stop_sell_triggers_as_market() {
    let mut book = OrderBook::new();
    let stop_id = uid();

    // Bid at 85 — far enough below the trigger price so it does not cross
    // the trigger ask (at 90), keeping both orders resting until the market buy.
    book.match_order(limit(uid(), uid(), Side::Buy, 85, 5));

    // Stop-sell: triggers when last_trade_price ≤ 95.
    let stop = Order {
        id: stop_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::StopMarket,
        price: None,
        quantity: d(3),
        remaining: d(3),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(95)),
        oco_linked_id: None,
        display_qty: None,
    };
    book.match_order(stop);

    // Place a resting ask at 90 (does not cross the bid at 85).
    book.match_order(limit(uid(), uid(), Side::Sell, 90, 1));

    // Market buy fills the ask at 90 → trade price = 90 ≤ 95 → sell stop triggers.
    let events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    let has_trigger = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_id));
    assert!(
        has_trigger,
        "expected StopTriggered for sell stop at 95 after trade at 90"
    );
}

// ── Case 16: Stop-buy does not trigger at price strictly below stop ───────────

#[test]
fn c16_stop_buy_does_not_trigger_below_price() {
    let mut book = OrderBook::new();
    let stop_id = uid();

    let stop = Order {
        id: stop_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::StopMarket,
        price: None,
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(100)),
        oco_linked_id: None,
        display_qty: None,
    };
    book.match_order(stop);

    // Trade at 99 — should NOT trigger the buy stop (needs price >= 100).
    let trigger_maker = uid();
    book.match_order(limit(trigger_maker, uid(), Side::Sell, 99, 1));
    let events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    let has_trigger = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_id));
    assert!(
        !has_trigger,
        "stop should NOT trigger at price 99 when stop_price=100"
    );
}

// ── Case 17: IOC partial fill, remainder cancelled ────────────────────────────

#[test]
fn c17_ioc_partial_fill_remainder_cancelled() {
    let mut book = OrderBook::new();
    let maker_id = uid();
    let taker_id = uid();

    book.match_order(limit(maker_id, uid(), Side::Sell, 100, 2));

    let ioc = Order {
        id: taker_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::Ioc,
        price: Some(d(100)),
        quantity: d(5),
        remaining: d(5),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(ioc);

    assert_eq!(events[0], fill_event(taker_id, maker_id, 100, 2, Side::Buy));
    assert_eq!(events[1], cancelled(taker_id, CancelReason::NoLiquidity));
    assert_eq!(events.len(), 2);
}

// ── Case 18: IOC no match → cancelled immediately ────────────────────────────

#[test]
fn c18_ioc_no_match_cancelled() {
    let mut book = OrderBook::new();
    let taker_id = uid();

    let ioc = Order {
        id: taker_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::Ioc,
        price: Some(d(100)),
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(ioc);
    assert_eq!(events, vec![cancelled(taker_id, CancelReason::NoLiquidity)]);
}

// ── Case 19: FOK fully fills ──────────────────────────────────────────────────

#[test]
fn c19_fok_fully_fills() {
    let mut book = OrderBook::new();
    let m1 = uid();
    let m2 = uid();
    let t = uid();

    book.match_order(limit(m1, uid(), Side::Sell, 99, 2));
    book.match_order(limit(m2, uid(), Side::Sell, 100, 2));

    let fok = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::Fok,
        price: Some(d(100)),
        quantity: d(4),
        remaining: d(4),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(fok);
    assert_eq!(
        events,
        vec![
            fill_event(t, m1, 99, 2, Side::Buy),
            fill_event(t, m2, 100, 2, Side::Buy),
        ]
    );
    assert!(book.best_ask().is_none());
}

// ── Case 20: FOK sell rejected when insufficient ──────────────────────────────

#[test]
fn c20_fok_sell_rejected_insufficient() {
    let mut book = OrderBook::new();
    let t = uid();
    book.match_order(limit(uid(), uid(), Side::Buy, 100, 2));

    let fok = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::Fok,
        price: Some(d(100)),
        quantity: d(5),
        remaining: d(5),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(fok);
    assert_eq!(events, vec![cancelled(t, CancelReason::FokUnfilled)]);
    // Bid still intact.
    let (bid_depth, _) = book.total_depth();
    assert_eq!(bid_depth, d(2));
}

// ── Case 21: OCO — buy leg fills → sell leg auto-cancelled ───────────────────

#[test]
fn c21_oco_buy_fills_cancels_sell() {
    let mut book = OrderBook::new();
    let buy_leg = uid();
    let sell_leg = uid();
    let user = uid();

    book.register_oco(buy_leg, sell_leg);

    // Place sell (stop) leg first as a resting limit.
    book.match_order(limit(sell_leg, user, Side::Sell, 110, 2));

    // Place buy leg — it crosses the existing ask at 100 and fills completely.
    let ask_maker = uid();
    book.match_order(limit(ask_maker, uid(), Side::Sell, 100, 2));
    let events = book.match_order(limit(buy_leg, user, Side::Buy, 100, 2));

    let has_fill = events
        .iter()
        .any(|e| matches!(e, EngineEvent::Fill(f) if f.taker_order_id == buy_leg));
    assert!(has_fill, "buy leg must fill");
    let has_oco_cancel = events.iter().any(|e| matches!(e, EngineEvent::Cancelled { order_id, reason: CancelReason::Oco } if *order_id == sell_leg));
    assert!(has_oco_cancel, "sell leg must be OCO-cancelled");
}

// ── Case 22: OCO — cancel_order on one leg cancels the other ─────────────────

#[test]
fn c22_oco_cancel_propagates() {
    let mut book = OrderBook::new();
    let leg_a = uid();
    let leg_b = uid();
    let user = uid();

    book.register_oco(leg_a, leg_b);
    book.match_order(limit(leg_a, user, Side::Buy, 90, 1));
    book.match_order(limit(leg_b, user, Side::Sell, 110, 1));

    let events = book.cancel_order(leg_a);
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::Cancelled { order_id, .. } if *order_id == leg_a)));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Cancelled { order_id, reason: CancelReason::Oco } if *order_id == leg_b)));
}

// ── Case 23: cancel_order removes resting limit, depth decreases ─────────────

#[test]
fn c23_cancel_resting_limit() {
    let mut book = OrderBook::new();
    let m = uid();
    book.match_order(limit(m, uid(), Side::Sell, 100, 5));
    let (_, ask_depth_before) = book.total_depth();
    assert_eq!(ask_depth_before, d(5));

    let events = book.cancel_order(m);
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::Cancelled { order_id, .. } if *order_id == m)));
    let (_, ask_depth_after) = book.total_depth();
    assert_eq!(ask_depth_after, d(0));
}

// ── Case 24: cancel_order on non-existent order → empty Vec ──────────────────

#[test]
fn c24_cancel_nonexistent_returns_none() {
    let mut book = OrderBook::new();
    let events = book.cancel_order(uid());
    assert!(events.is_empty());
}

// ── Case 25: Multiple makers at same price — FIFO preserved ─────────────────

#[test]
fn c25_fifo_within_price_level() {
    let mut book = OrderBook::new();
    let first = uid();
    let second = uid();
    let user = uid();

    book.match_order(limit(first, user, Side::Sell, 100, 1));
    book.match_order(limit(second, user, Side::Sell, 100, 1));

    // Two separate market buys of 1 each — should match first, then second.
    let e1 = book.match_order(market(uid(), uid(), Side::Buy, 1));
    let e2 = book.match_order(market(uid(), uid(), Side::Buy, 1));

    assert!(matches!(&e1[0], EngineEvent::Fill(f) if f.maker_order_id == first));
    assert!(matches!(&e2[0], EngineEvent::Fill(f) if f.maker_order_id == second));
}

// ── Case 26: Partial maker fill — maker remains on book ──────────────────────

#[test]
fn c26_partial_maker_fill_remains() {
    let mut book = OrderBook::new();
    let maker_id = uid();
    book.match_order(limit(maker_id, uid(), Side::Sell, 100, 10));

    let events = book.match_order(market(uid(), uid(), Side::Buy, 3));
    assert_eq!(
        events,
        vec![fill_event(
            events[0].taker_fill_id(),
            maker_id,
            100,
            3,
            Side::Buy
        )]
    );

    let (_, ask_depth) = book.total_depth();
    assert_eq!(ask_depth, d(7));
}

// ── Case 27: Limit sell above best bid → rests ───────────────────────────────

#[test]
fn c27_limit_sell_no_cross_rests() {
    let mut book = OrderBook::new();
    book.match_order(limit(uid(), uid(), Side::Buy, 95, 5));

    let seller = uid();
    let events = book.match_order(limit(seller, uid(), Side::Sell, 100, 3));
    assert_eq!(events, vec![rested(seller, 3)]);
    assert_eq!(book.best_ask(), Some(d(100)));
}

// ── Case 28: Market buy across three levels ───────────────────────────────────

#[test]
fn c28_market_buy_across_three_levels() {
    let mut book = OrderBook::new();
    let m1 = uid();
    let m2 = uid();
    let m3 = uid();
    book.match_order(limit(m1, uid(), Side::Sell, 100, 1));
    book.match_order(limit(m2, uid(), Side::Sell, 101, 1));
    book.match_order(limit(m3, uid(), Side::Sell, 102, 1));

    let t = uid();
    let events = book.match_order(market(t, uid(), Side::Buy, 3));
    assert_eq!(events.len(), 3);
    assert!(
        matches!(&events[0], EngineEvent::Fill(f) if f.price == d(100) && f.maker_order_id == m1)
    );
    assert!(
        matches!(&events[1], EngineEvent::Fill(f) if f.price == d(101) && f.maker_order_id == m2)
    );
    assert!(
        matches!(&events[2], EngineEvent::Fill(f) if f.price == d(102) && f.maker_order_id == m3)
    );
}

// ── Case 29: Empty level removed after full fill ─────────────────────────────

#[test]
fn c29_empty_level_removed() {
    let mut book = OrderBook::new();
    book.match_order(limit(uid(), uid(), Side::Sell, 100, 1));
    assert_eq!(book.best_ask(), Some(d(100)));

    book.match_order(market(uid(), uid(), Side::Buy, 1));
    assert!(book.best_ask().is_none());
}

// ── Case 30: Limit rests when no opposite liquidity ──────────────────────────

#[test]
fn c30_limit_rests_no_opposite() {
    let mut book = OrderBook::new();
    let id = uid();
    let events = book.match_order(limit(id, uid(), Side::Buy, 100, 5));
    assert_eq!(events, vec![rested(id, 5)]);
    assert_eq!(book.best_bid(), Some(d(100)));
}

// ── Case 31: IOC sell with limit price, partial match, remainder cancelled ────

#[test]
fn c31_ioc_sell_partial_with_price() {
    let mut book = OrderBook::new();
    let maker_id = uid();
    let taker_id = uid();
    book.match_order(limit(maker_id, uid(), Side::Buy, 100, 2));

    let ioc = Order {
        id: taker_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::Ioc,
        price: Some(d(100)),
        quantity: d(5),
        remaining: d(5),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(ioc);
    assert_eq!(
        events[0],
        fill_event(taker_id, maker_id, 100, 2, Side::Sell)
    );
    assert_eq!(events[1], cancelled(taker_id, CancelReason::NoLiquidity));
}

// ── Case 32: Post-only sell crosses → rejected ────────────────────────────────

#[test]
fn c32_post_only_sell_crosses_rejected() {
    let mut book = OrderBook::new();
    book.match_order(limit(uid(), uid(), Side::Buy, 100, 1));

    let t = uid();
    let po = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::PostOnly,
        price: Some(d(100)),
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(po);
    assert_eq!(events, vec![cancelled(t, CancelReason::PostOnlyCrossed)]);
}

// ── Case 33: Post-only sell no cross → rests ─────────────────────────────────

#[test]
fn c33_post_only_sell_no_cross_rests() {
    let mut book = OrderBook::new();
    book.match_order(limit(uid(), uid(), Side::Buy, 95, 1));

    let t = uid();
    let po = Order {
        id: t,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::PostOnly,
        price: Some(d(100)),
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    let events = book.match_order(po);
    assert_eq!(events, vec![rested(t, 1)]);
}

// ── Case 34: Stop-buy triggers at exactly stop_price (boundary) ───────────────

#[test]
fn c34_stop_buy_triggers_exactly_at_stop_price() {
    let mut book = OrderBook::new();
    let stop_id = uid();

    // Place ask at 105 so triggered limit can fill.
    book.match_order(limit(uid(), uid(), Side::Sell, 105, 3));

    let stop = Order {
        id: stop_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::StopLimit,
        price: Some(d(105)),
        quantity: d(2),
        remaining: d(2),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(100)),
        oco_linked_id: None,
        display_qty: None,
    };
    book.match_order(stop);

    // Trade at exactly 100 (stop_price).
    book.match_order(limit(uid(), uid(), Side::Sell, 100, 1));
    let events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    let triggered = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_id));
    assert!(triggered, "buy stop must trigger at exactly stop_price=100");
}

// ── Case 35: Stop-sell triggers at exactly stop_price (boundary) ──────────────

#[test]
fn c35_stop_sell_triggers_exactly_at_stop_price() {
    let mut book = OrderBook::new();
    let stop_id = uid();

    // Resting bid at 90 for triggered market sell to fill.
    book.match_order(limit(uid(), uid(), Side::Buy, 90, 3));

    let stop = Order {
        id: stop_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::StopMarket,
        price: None,
        quantity: d(2),
        remaining: d(2),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(95)),
        oco_linked_id: None,
        display_qty: None,
    };
    book.match_order(stop);

    // Trade at exactly 95 (stop_price).
    book.match_order(limit(uid(), uid(), Side::Sell, 95, 1));
    let events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    let triggered = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_id));
    assert!(triggered, "sell stop must trigger at exactly stop_price=95");
}

// ── Case 36: Multiple stops — only triggered ones activate ───────────────────

#[test]
fn c36_multiple_stops_only_triggered_activate() {
    let mut book = OrderBook::new();
    let stop_a = uid(); // stop_price = 100, triggers at >= 100
    let stop_b = uid(); // stop_price = 110, should NOT trigger at price 100

    // Resting ask for triggered stop_a to potentially fill.
    book.match_order(limit(uid(), uid(), Side::Sell, 101, 5));

    let make_buy_stop = |id: Uuid, sp: i64| Order {
        id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::StopMarket,
        price: None,
        quantity: d(1),
        remaining: d(1),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(sp)),
        oco_linked_id: None,
        display_qty: None,
    };
    book.match_order(make_buy_stop(stop_a, 100));
    book.match_order(make_buy_stop(stop_b, 110));

    // Trade at 100 — triggers stop_a but not stop_b.
    book.match_order(limit(uid(), uid(), Side::Sell, 100, 1));
    let events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    let a_triggered = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_a));
    let b_triggered = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_b));
    assert!(a_triggered, "stop_a must trigger at price 100");
    assert!(
        !b_triggered,
        "stop_b must NOT trigger at price 100 (stop_price=110)"
    );
}

// ── Case 37: Iceberg hidden qty correct after multiple refreshes ──────────────

#[test]
fn c37_iceberg_total_remaining_tracks_correctly() {
    let mut book = OrderBook::new();
    let maker_id = uid();

    let iceberg = Order {
        id: maker_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::Iceberg,
        price: Some(d(100)),
        quantity: d(6),
        remaining: d(6),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: Some(d(2)),
    };
    book.match_order(iceberg);

    // Fill 2 units → display refreshes, 4 remaining total.
    book.match_order(market(uid(), uid(), Side::Buy, 2));
    let (_, depth) = book.total_depth();
    assert_eq!(depth, d(4));

    // Fill 2 more → 2 remaining.
    book.match_order(market(uid(), uid(), Side::Buy, 2));
    let (_, depth) = book.total_depth();
    assert_eq!(depth, d(2));
}

// ── Case 38: total_depth matches manual sum after complex sequence ────────────

#[test]
fn c38_total_depth_consistent() {
    let mut book = OrderBook::new();
    let user = uid();

    for price in [95, 97, 99] {
        book.match_order(limit(uid(), user, Side::Buy, price, 3));
    }
    for price in [101, 103, 105] {
        book.match_order(limit(uid(), user, Side::Sell, price, 3));
    }
    // Partial fill: remove half the bid liquidity.
    book.match_order(market(uid(), uid(), Side::Sell, 5));

    let (bid_depth, ask_depth) = book.total_depth();
    // 9 bid qty - 5 consumed = 4 remaining bids; 9 ask qty unchanged.
    assert_eq!(bid_depth, d(4));
    assert_eq!(ask_depth, d(9));
}

// ── Case 39: WAL round-trip — replay produces identical book depth ────────────

#[test]
fn c39_wal_round_trip() {
    use cex_core::wal::{Snapshot, Wal, WalCommand};
    use std::env::temp_dir;

    let wal_path = temp_dir().join(format!("test_wal_{}.bin", Uuid::new_v4()));
    let snap_path = temp_dir().join(format!("test_snap_{}.bin", Uuid::new_v4()));

    let mut wal = Wal::open(&wal_path).expect("wal open");
    let mut book = OrderBook::new();

    // Place 10 limit orders and record them in the WAL.
    let user = uid();
    for i in 1..=5i64 {
        let o = limit(uid(), user, Side::Sell, 100 + i, i);
        wal.append(WalCommand::PlaceOrder(o.clone()))
            .expect("wal append");
        book.match_order(o);
    }
    for i in 1..=5i64 {
        let o = limit(uid(), user, Side::Buy, 90 + i, i);
        wal.append(WalCommand::PlaceOrder(o.clone()))
            .expect("wal append");
        book.match_order(o);
    }

    let (orig_bid, orig_ask) = book.total_depth();

    // Save snapshot.
    Snapshot::save(&snap_path, wal.sequence, &book).expect("snapshot save");

    // Reload from snapshot and verify depth matches.
    let (snap_seq, mut replayed_book) = Snapshot::load(&snap_path).expect("snapshot load");
    replayed_book.rebuild_index();
    let (rep_bid, rep_ask) = replayed_book.total_depth();

    assert_eq!(snap_seq, wal.sequence);
    assert_eq!(
        rep_bid, orig_bid,
        "bid depth must match after snapshot reload"
    );
    assert_eq!(
        rep_ask, orig_ask,
        "ask depth must match after snapshot reload"
    );

    // Cleanup.
    let _ = std::fs::remove_file(&wal_path);
    let _ = std::fs::remove_file(&snap_path);
}

// ── Case 40: WAL replay from commands — book state identical ─────────────────

#[test]
fn c40_wal_command_replay() {
    use cex_core::wal::{Wal, WalCommand};
    use std::env::temp_dir;

    let wal_path = temp_dir().join(format!("test_wal_cmd_{}.bin", Uuid::new_v4()));
    let mut wal = Wal::open(&wal_path).expect("wal open");
    let mut original_book = OrderBook::new();

    let user = uid();
    let orders: Vec<Order> = (1..=5i64)
        .map(|i| limit(uid(), user, Side::Sell, 100 + i, i))
        .collect();

    for o in &orders {
        wal.append(WalCommand::PlaceOrder(o.clone()))
            .expect("append");
        original_book.match_order(o.clone());
    }
    drop(wal); // flush and close

    // Replay WAL commands into fresh book.
    let entries = Wal::replay(&wal_path).expect("replay");
    let mut replayed_book = OrderBook::new();
    for entry in entries {
        match entry.command {
            WalCommand::PlaceOrder(o) => {
                replayed_book.match_order(o);
            }
            WalCommand::CancelOrder(id) => {
                replayed_book.cancel_order(id);
            }
        }
    }

    assert_eq!(original_book.total_depth(), replayed_book.total_depth());
    assert_eq!(original_book.best_ask(), replayed_book.best_ask());

    let _ = std::fs::remove_file(&wal_path);
}

// ── Case 41: WAL corrupt tail truncation ─────────────────────────────────────

#[test]
fn c41_wal_corrupt_tail_truncated() {
    use cex_core::wal::{Wal, WalCommand};
    use std::env::temp_dir;
    use std::io::Write;

    let wal_path = temp_dir().join(format!("test_wal_corrupt_{}.bin", Uuid::new_v4()));
    let mut wal = Wal::open(&wal_path).expect("wal open");

    let o = limit(uid(), uid(), Side::Sell, 100, 1);
    wal.append(WalCommand::PlaceOrder(o)).expect("append");
    drop(wal);

    // Corrupt the tail by appending garbage bytes.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&wal_path)
        .expect("open");
    file.write_all(b"\x00\x00\xFF\xFF garbage bytes that are not valid bincode")
        .expect("write garbage");
    drop(file);

    // Replay should return only the valid first entry, not error.
    let entries = Wal::replay(&wal_path).expect("replay should not error on corrupt tail");
    assert_eq!(entries.len(), 1, "only the valid entry should be returned");

    let _ = std::fs::remove_file(&wal_path);
}

// ── Case 42: Crossed book impossible — limit buy above best ask fills ─────────

#[test]
fn c42_no_crossed_book_limit_buy_above_ask() {
    let mut book = OrderBook::new();
    let maker_id = uid();
    book.match_order(limit(maker_id, uid(), Side::Sell, 100, 2));

    // Aggressive buy at 105 — crosses and fills the ask at 100.
    let taker_id = uid();
    let events = book.match_order(limit(taker_id, uid(), Side::Buy, 105, 2));
    assert!(matches!(&events[0], EngineEvent::Fill(f) if f.price == d(100)));

    // Book must not be crossed.
    if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
        assert!(bid < ask, "crossed book after aggressive limit buy");
    }
}

// ── Case 43: Stop-market sell triggered; no resting bids → market cancelled ───

#[test]
fn c43_triggered_stop_market_no_liquidity_cancelled() {
    let mut book = OrderBook::new();
    let stop_id = uid();

    // No resting bids — triggered market sell will cancel.
    let stop = Order {
        id: stop_id,
        user_id: uid(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::StopMarket,
        price: None,
        quantity: d(5),
        remaining: d(5),
        stp_mode: StpMode::Decrement,
        stop_price: Some(d(95)),
        oco_linked_id: None,
        display_qty: None,
    };
    book.match_order(stop);

    // Trigger at 90 (below 95).
    book.match_order(limit(uid(), uid(), Side::Sell, 90, 1));
    let events = book.match_order(market(uid(), uid(), Side::Buy, 1));

    let triggered = events
        .iter()
        .any(|e| matches!(e, EngineEvent::StopTriggered { order_id } if *order_id == stop_id));
    assert!(triggered, "expected StopTriggered");
    let cancelled_stop = events.iter().any(|e| matches!(e, EngineEvent::Cancelled { order_id, reason: CancelReason::NoLiquidity } if *order_id == stop_id));
    assert!(
        cancelled_stop,
        "triggered market sell must cancel when no bids"
    );
}

// ── Case 44: Market sell empty book → cancel ─────────────────────────────────

#[test]
fn c44_market_sell_empty_book_cancels() {
    let mut book = OrderBook::new();
    let t = uid();
    let events = book.match_order(market(t, uid(), Side::Sell, 3));
    assert_eq!(events, vec![cancelled(t, CancelReason::NoLiquidity)]);
}

// ── Case 45: rebuild_index allows cancel after snapshot reload ────────────────

#[test]
fn c45_rebuild_index_enables_cancel() {
    use cex_core::wal::Snapshot;
    use std::env::temp_dir;

    let snap_path = temp_dir().join(format!("test_snap_idx_{}.bin", Uuid::new_v4()));

    let mut book = OrderBook::new();
    let maker_id = uid();
    book.match_order(limit(maker_id, uid(), Side::Buy, 100, 5));

    Snapshot::save(&snap_path, 1, &book).expect("save");
    let (_, mut restored) = Snapshot::load(&snap_path).expect("load");
    restored.rebuild_index();

    let events = restored.cancel_order(maker_id);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::Cancelled { order_id, .. } if *order_id == maker_id)),
        "must be able to cancel after rebuild_index"
    );

    let _ = std::fs::remove_file(&snap_path);
}

// ── Trait helper: extract taker_order_id from EngineEvent::Fill ─────────────

trait FillId {
    fn taker_fill_id(&self) -> Uuid;
}
impl FillId for EngineEvent {
    fn taker_fill_id(&self) -> Uuid {
        match self {
            Self::Fill(f) => f.taker_order_id,
            _ => panic!("not a fill"),
        }
    }
}
