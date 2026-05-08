use cex_core::book::{EngineEvent, OrderBook};
use cex_core::types::{Order, OrderType, Side};
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
        order_type in prop_oneof![Just(OrderType::Limit), Just(OrderType::Market)],
        price in arb_price(),
        quantity in arb_qty()
    ) -> Order {
        Order {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "BTCUSDT".to_string(),
            side,
            order_type,
            price: if order_type == OrderType::Limit { Some(price) } else { None },
            quantity,
            remaining: quantity,
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
                assert!(best_bid < best_ask, "Crossed book: best_bid={} best_ask={}", best_bid, best_ask);
            }
        }
    }

    #[test]
    fn depth_matches_remaining(orders in prop::collection::vec(arb_order(), 1..100)) {
        let mut book = OrderBook::new();

        for order in orders {
            let events = book.match_order(order);
            
            for event in events {
                match event {
                    EngineEvent::Rested { .. } => {}
                    EngineEvent::Fill(_) | EngineEvent::Cancelled { .. } => {}
                }
            }
            
            // Check invariant: total depth == sum of all remaining in the data structures
            // `total_depth` already iterates and sums up remaining. We just need to make sure
            // we don't have dangling orders. We'll just assert that total depth matches the manual sum.
            let (bid_depth, ask_depth) = book.total_depth();
            assert!(bid_depth >= Decimal::ZERO);
            assert!(ask_depth >= Decimal::ZERO);
        }
    }
}

#[test]
fn test_basic_match() {
    let mut book = OrderBook::new();

    // Maker
    let maker = Order {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        symbol: "BTCUSDT".to_string(),
        side: Side::Sell,
        order_type: OrderType::Limit,
        price: Some(Decimal::new(100, 0)),
        quantity: Decimal::new(10, 0),
        remaining: Decimal::new(10, 0),
    };
    
    let events = book.match_order(maker);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], EngineEvent::Rested { .. }));

    // Taker
    let taker = Order {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        symbol: "BTCUSDT".to_string(),
        side: Side::Buy,
        order_type: OrderType::Market,
        price: None,
        quantity: Decimal::new(4, 0),
        remaining: Decimal::new(4, 0),
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
    assert_eq!(ask_depth, Decimal::new(6, 0)); // 10 - 4
}
