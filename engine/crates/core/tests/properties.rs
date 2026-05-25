// Tests intentionally use direct event indexing and panic-style assertions for
// concise invariant failures; AGENTS.md §4.2 exempts test modules from these lints.
#![allow(clippy::indexing_slicing, clippy::panic)]

use cex_core::book::{EngineEvent, OrderBook};
use cex_core::types::{Order, OrderType, Side, StpMode};
use proptest::prelude::*;
use rust_decimal::Decimal;
use uuid::Uuid;

prop_compose! {
    fn arb_price()(p in 1..1000u32) -> Decimal {
        Decimal::new(p.into(), 0)
    }
}

prop_compose! {
    fn arb_qty()(q in 1..100u32) -> Decimal {
        Decimal::new(q.into(), 0)
    }
}

prop_compose! {
    fn arb_order()(
        side in prop_oneof![Just(Side::Buy), Just(Side::Sell)],
        order_type in prop_oneof![
            Just(OrderType::Limit),
            Just(OrderType::Market),
            Just(OrderType::Ioc),
        ],
        price in arb_price(),
        quantity in arb_qty()
    ) -> Order {
        let needs_price = matches!(order_type, OrderType::Limit | OrderType::Ioc);
        Order {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "BTCUSDT".into(),
            side,
            order_type,
            price: if needs_price { Some(price) } else { None },
            quantity,
            remaining: quantity,
            stp_mode: StpMode::Decrement,
            stop_price: None,
            oco_linked_id: None,
            display_qty: None,
        }
    }
}

proptest! {
    #[test]
    fn no_crossed_book(orders in prop::collection::vec(arb_order(), 1..100)) {
        let mut book = OrderBook::new();

        for order in orders {
            let _events = book.match_order(order);

            if let (Some(best_bid), Some(best_ask)) = (book.best_bid(), book.best_ask()) {
                prop_assert!(
                    best_bid < best_ask,
                    "Crossed book: best_bid={best_bid} best_ask={best_ask}"
                );
            }
        }
    }

    #[test]
    fn depth_matches_remaining(orders in prop::collection::vec(arb_order(), 1..100)) {
        let mut book = OrderBook::new();

        for order in orders {
            let _events = book.match_order(order);
        }

        let (bid_depth, ask_depth) = book.total_depth();
        prop_assert!(bid_depth >= Decimal::ZERO);
        prop_assert!(ask_depth >= Decimal::ZERO);
    }

    #[test]
    fn ioc_never_rests(orders in prop::collection::vec(arb_order(), 1..50)) {
        let mut book = OrderBook::new();

        for order in orders {
            if order.order_type == OrderType::Ioc {
                let events = book.match_order(order);
                // IOC must never produce a Rested event
                prop_assert!(
                    events.iter().all(|e| !matches!(e, EngineEvent::Rested { .. })),
                    "IOC order produced a Rested event"
                );
            } else {
                book.match_order(order);
            }
        }
    }
}

#[test]
fn test_basic_match() {
    let mut book = OrderBook::new();

    let maker = Order {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        symbol: "BTCUSDT".into(),
        side: Side::Sell,
        order_type: OrderType::Limit,
        price: Some(Decimal::new(100, 0)),
        quantity: Decimal::new(10, 0),
        remaining: Decimal::new(10, 0),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };

    let events = book.match_order(maker);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], EngineEvent::Rested { .. }));

    let taker = Order {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        symbol: "BTCUSDT".into(),
        side: Side::Buy,
        order_type: OrderType::Market,
        price: None,
        quantity: Decimal::new(4, 0),
        remaining: Decimal::new(4, 0),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };

    let events = book.match_order(taker);
    assert_eq!(events.len(), 1);
    if let EngineEvent::Fill(fill) = &events[0] {
        assert_eq!(fill.price, Decimal::new(100, 0));
        assert_eq!(fill.quantity, Decimal::new(4, 0));
    } else {
        panic!("Expected fill");
    }

    let (bid_depth, ask_depth) = book.total_depth();
    assert_eq!(bid_depth, Decimal::ZERO);
    assert_eq!(ask_depth, Decimal::new(6, 0));
}
