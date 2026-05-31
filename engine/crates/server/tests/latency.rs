//! Engine TCP round-trip latency measurement.
//!
//! Starts an in-process engine server (same approach as tcp_server.rs integration tests),
//! measures ping and order-placement round-trip latency over 10,000 requests each,
//! and prints a statistical summary.
//!
//! Run with:  cargo test --release -p cex-server --test latency -- --nocapture
//!
//! The `#[allow]` below mirrors the exemptions in tcp_server.rs (AGENTS.md §4.2).
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::pedantic,
    clippy::nursery
)]

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Instant;

use cex_proto::{
    read_json_frame, write_json_frame, EngineOrder, EngineOrderType, EngineRequest, EngineResponse,
    OrderSide, PingRequest, PlaceRequest, StpMode,
};
use cex_server::{EngineServer, EngineServerConfig};
use rust_decimal::Decimal;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const ITERATIONS: usize = 10_000;

struct RunningServer {
    addr: SocketAddr,
    dir: PathBuf,
    handle: JoinHandle<Result<(), cex_server::ServerError>>,
    token: CancellationToken,
}

impl RunningServer {
    async fn shutdown(self) {
        self.token.cancel();
        let _ = self.handle.await;
        let _ = fs::remove_dir_all(self.dir);
    }
}

async fn start_server() -> RunningServer {
    let dir = std::env::temp_dir().join(format!("aether-latency-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("temp dir created");
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");
    let token = CancellationToken::new();
    let config = EngineServerConfig {
        bind_addr: addr,
        command_wal_path: dir.join("commands.wal"),
        event_log_path: dir.join("events.wal"),
        snapshot_depth: 50,
    };
    let server = EngineServer::new(config);
    let server_token = token.clone();
    let handle = tokio::spawn(async move { server.run_listener(listener, server_token).await });
    RunningServer { addr, dir, handle, token }
}

/// Reads frames from the stream, skipping unsolicited Event broadcasts,
/// until a non-Event response arrives.
async fn read_response(stream: &mut TcpStream) -> EngineResponse {
    loop {
        let resp: EngineResponse = read_json_frame(stream)
            .await
            .expect("frame read")
            .expect("frame present");
        if !matches!(resp, EngineResponse::Event(_)) {
            return resp;
        }
    }
}

fn stats_us(samples: &[u128]) -> (f64, f64, f64, f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let mean = sorted.iter().sum::<u128>() as f64 / n as f64 / 1_000.0;
    let min  = sorted[0] as f64 / 1_000.0;
    let max  = sorted[n - 1] as f64 / 1_000.0;
    let p50  = sorted[n / 2] as f64 / 1_000.0;
    let p99  = sorted[(n * 99) / 100] as f64 / 1_000.0;
    (min, mean, p50, p99, max)
}

#[tokio::test]
async fn measure_ping_rtt() {
    let server = start_server().await;
    let mut stream = TcpStream::connect(server.addr).await.expect("connected");
    let mut samples = Vec::with_capacity(ITERATIONS);

    // Warm-up: 200 pings before measuring.
    for _ in 0..200 {
        write_json_frame(
            &mut stream,
            &EngineRequest::Ping(PingRequest { request_id: Uuid::new_v4() }),
        )
        .await
        .expect("write");
        read_response(&mut stream).await;
    }

    for _ in 0..ITERATIONS {
        let t0 = Instant::now();
        write_json_frame(
            &mut stream,
            &EngineRequest::Ping(PingRequest { request_id: Uuid::new_v4() }),
        )
        .await
        .expect("write");
        read_response(&mut stream).await;
        samples.push(t0.elapsed().as_nanos());
    }

    let (min, mean, p50, p99, max) = stats_us(&samples);
    println!("\n─── Engine TCP Ping RTT ({ITERATIONS} iterations) ───────────────");
    println!("  min    {min:.2} µs");
    println!("  mean   {mean:.2} µs");
    println!("  p50    {p50:.2} µs");
    println!("  p99    {p99:.2} µs");
    println!("  max    {max:.2} µs");
    println!("  throughput  {:.0}k req/s", 1_000_000.0 / mean / 1_000.0);
    println!("────────────────────────────────────────────────────────\n");

    server.shutdown().await;
}

#[tokio::test]
async fn measure_place_order_rtt() {
    let server = start_server().await;
    let mut stream = TcpStream::connect(server.addr).await.expect("connected");
    let mut samples = Vec::with_capacity(ITERATIONS);

    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();

    // Warm-up: seed a thin book so place requests find a counterparty path.
    for _ in 0..200 {
        let side = OrderSide::Buy;
        let price = Decimal::new(10_000, 0);
        write_json_frame(
            &mut stream,
            &EngineRequest::Place(PlaceRequest {
                request_id: Uuid::new_v4(),
                order: EngineOrder {
                    id: Uuid::new_v4(),
                    user_id: user_a,
                    symbol: "BTCUSDT".to_owned(),
                    side,
                    order_type: EngineOrderType::Limit,
                    price: Some(price),
                    quantity: Decimal::new(1, 0),
                    stp_mode: StpMode::Decrement,
                    stop_price: None,
                    oco_linked_id: None,
                    display_qty: None,
                },
            }),
        )
        .await
        .expect("write");
        read_response(&mut stream).await;
    }

    // Measure: alternate buy/sell limit orders so some rest, some match.
    for i in 0..ITERATIONS {
        let (side, user, price) = if i % 2 == 0 {
            (OrderSide::Buy, user_a, Decimal::new(9_000, 0))
        } else {
            (OrderSide::Sell, user_b, Decimal::new(9_500, 0))
        };
        let t0 = Instant::now();
        write_json_frame(
            &mut stream,
            &EngineRequest::Place(PlaceRequest {
                request_id: Uuid::new_v4(),
                order: EngineOrder {
                    id: Uuid::new_v4(),
                    user_id: user,
                    symbol: "BTCUSDT".to_owned(),
                    side,
                    order_type: EngineOrderType::Limit,
                    price: Some(price),
                    quantity: Decimal::new(1, 0),
                    stp_mode: StpMode::Decrement,
                    stop_price: None,
                    oco_linked_id: None,
                    display_qty: None,
                },
            }),
        )
        .await
        .expect("write");
        read_response(&mut stream).await;
        samples.push(t0.elapsed().as_nanos());
    }

    let (min, mean, p50, p99, max) = stats_us(&samples);
    println!("\n─── Engine TCP Place-Order RTT ({ITERATIONS} iterations) ────────");
    println!("  min    {min:.2} µs");
    println!("  mean   {mean:.2} µs");
    println!("  p50    {p50:.2} µs");
    println!("  p99    {p99:.2} µs");
    println!("  max    {max:.2} µs");
    println!("  throughput  {:.0}k req/s", 1_000_000.0 / mean / 1_000.0);
    println!("────────────────────────────────────────────────────────\n");

    server.shutdown().await;
}
