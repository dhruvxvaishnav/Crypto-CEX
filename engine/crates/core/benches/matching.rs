use cex_core::book::OrderBook;
use cex_core::types::{Order, OrderType, Side, StpMode};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use uuid::Uuid;

fn generate_random_orders(count: usize) -> Vec<Order> {
    let mut orders = Vec::with_capacity(count);
    // Simple deterministic PRNG — no SystemTime on the hot path (AGENTS.md §3.5).
    let mut seed = 12_345_u32;
    let mut next_rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };

    for _ in 0..count {
        let side = if next_rand() % 2 == 0 {
            Side::Buy
        } else {
            Side::Sell
        };
        let price_offset = next_rand() % 200;
        let price = Decimal::new(i64::from(9900 + price_offset), 0);
        let quantity = Decimal::new(i64::from((next_rand() % 10) + 1), 0);

        orders.push(Order {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            symbol: "BTCUSDT".into(),
            side,
            order_type: OrderType::Limit,
            price: Some(price),
            quantity,
            remaining: quantity,
            stp_mode: StpMode::Decrement,
            stop_price: None,
            oco_linked_id: None,
            display_qty: None,
        });
    }
    orders
}

/// Build a book with exactly `levels` bids and `levels` asks, one order per level.
/// This gives a controlled ~2×levels depth for the snapshot bench.
fn build_book_with_depth(levels: usize) -> OrderBook {
    let mut book = OrderBook::new();
    let user = Uuid::new_v4();
    // Bids at 9_000..9_000+levels (below asks to avoid crossing).
    for i in 0..levels {
        let price = Decimal::new(i64::try_from(9_000 + i).unwrap_or(i64::MAX), 0);
        book.match_order(Order {
            id: Uuid::new_v4(),
            user_id: user,
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(price),
            quantity: Decimal::new(5, 0),
            remaining: Decimal::new(5, 0),
            stp_mode: StpMode::Decrement,
            stop_price: None,
            oco_linked_id: None,
            display_qty: None,
        });
    }
    // Asks at 10_000..10_000+levels (above bids).
    for i in 0..levels {
        let price = Decimal::new(i64::try_from(10_000 + i).unwrap_or(i64::MAX), 0);
        book.match_order(Order {
            id: Uuid::new_v4(),
            user_id: user,
            symbol: "BTCUSDT".into(),
            side: Side::Sell,
            order_type: OrderType::Limit,
            price: Some(price),
            quantity: Decimal::new(5, 0),
            remaining: Decimal::new(5, 0),
            stp_mode: StpMode::Decrement,
            stop_price: None,
            oco_linked_id: None,
            display_qty: None,
        });
    }
    book
}

fn bench_match_random_limit_orders(c: &mut Criterion) {
    let orders = generate_random_orders(10_000);

    c.bench_function("bench_match_random_limit_orders", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();
            for order in orders.iter() {
                black_box(book.match_order(order.clone()));
            }
            black_box(book);
        });
    });
}

fn bench_book_snapshot_depth_100(c: &mut Criterion) {
    // Exactly 100 bid levels + 100 ask levels — no random crossing.
    let book = build_book_with_depth(100);

    c.bench_function("bench_book_snapshot_depth_100", |b| {
        b.iter(|| {
            let snapshot = book.clone();
            black_box(snapshot);
        });
    });
}

criterion_group!(
    benches,
    bench_match_random_limit_orders,
    bench_book_snapshot_depth_100
);
criterion_main!(benches);
