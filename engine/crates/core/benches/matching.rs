use cex_core::book::OrderBook;
use cex_core::types::{Order, OrderType, Side};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use uuid::Uuid;

fn generate_random_orders(count: usize) -> Vec<Order> {
    let mut orders = Vec::with_capacity(count);
    // Simple deterministic PRNG for stable benchmarks
    let mut seed = 12345u32;
    let mut next_rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };

    for _ in 0..count {
        let side = if next_rand() % 2 == 0 { Side::Buy } else { Side::Sell };
        // prices concentrated around 10000
        let price_offset = next_rand() % 200;
        let price = Decimal::new((9900 + price_offset).into(), 0);
        let quantity = Decimal::new(((next_rand() % 10) + 1).into(), 0);

        orders.push(Order {
            id: Uuid::new_v4(), // Uuid generation might be slow, but it's outside the hot path in the bench map
            user_id: Uuid::new_v4(),
            symbol: "BTCUSDT".to_string(),
            side,
            order_type: OrderType::Limit,
            price: Some(price),
            quantity,
            remaining: quantity,
        });
    }
    orders
}

fn bench_match_random_limit_orders(c: &mut Criterion) {
    let orders = generate_random_orders(10_000);
    
    c.bench_function("bench_match_random_limit_orders", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();
            for order in orders.iter() {
                // We use black_box to ensure the compiler doesn't optimize away the match
                black_box(book.match_order(order.clone()));
            }
            black_box(book);
        })
    });
}

fn bench_book_snapshot_depth_100(c: &mut Criterion) {
    // Generate a static book with 100 levels of depth on both sides
    let mut book = OrderBook::new();
    let orders = generate_random_orders(5000);
    for order in orders {
        book.match_order(order);
    }
    
    c.bench_function("bench_book_snapshot_depth_100", |b| {
        b.iter(|| {
            let snapshot = book.clone();
            black_box(snapshot);
        })
    });
}

criterion_group!(benches, bench_match_random_limit_orders, bench_book_snapshot_depth_100);
criterion_main!(benches);
