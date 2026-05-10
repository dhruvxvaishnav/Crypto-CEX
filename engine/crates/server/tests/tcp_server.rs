use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use cex_proto::{
    read_json_frame, write_json_frame, AckResult, CancelRequest, EngineOrder, EngineOrderType,
    EngineRequest, EngineResponse, OrderSide, OrderStatus, PingRequest, PlaceRequest,
    SnapshotRequest, StpMode,
};
use cex_server::{EngineServer, EngineServerConfig};
use rust_decimal::Decimal;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct RunningServer {
    addr: SocketAddr,
    dir: PathBuf,
    handle: JoinHandle<Result<(), cex_server::ServerError>>,
    token: CancellationToken,
}

impl RunningServer {
    async fn shutdown(self) {
        self.token.cancel();
        self.handle
            .await
            .expect("server task joins")
            .expect("server exits cleanly");
        fs::remove_dir_all(self.dir).expect("test directory removed");
    }
}

#[tokio::test]
async fn ping_round_trips_over_framed_tcp() {
    let server = start_server().await;
    let mut stream = TcpStream::connect(server.addr)
        .await
        .expect("client connects");
    let request_id = Uuid::new_v4();

    write_json_frame(
        &mut stream,
        &EngineRequest::Ping(PingRequest { request_id }),
    )
    .await
    .expect("ping request writes");
    let response = read_response(&mut stream).await;

    assert!(matches!(
        response,
        EngineResponse::Pong(pong) if pong.request_id == request_id
    ));
    server.shutdown().await;
}

#[tokio::test]
async fn place_limit_order_rests_and_appears_in_snapshot() {
    let server = start_server().await;
    let mut stream = TcpStream::connect(server.addr)
        .await
        .expect("client connects");
    let order_id = Uuid::new_v4();

    send_place_limit(&mut stream, order_id)
        .await
        .expect("place request writes");
    let response = read_response(&mut stream).await;

    match response {
        EngineResponse::Ack(ack) => match ack.result {
            AckResult::Order(order) => {
                assert_eq!(order.order_id, order_id);
                assert_eq!(order.status, OrderStatus::New);
            }
            other => panic!("expected order ack, got {other:?}"),
        },
        other => panic!("expected ack response, got {other:?}"),
    }

    write_json_frame(
        &mut stream,
        &EngineRequest::Snapshot(SnapshotRequest {
            request_id: Uuid::new_v4(),
            symbol: "BTCUSDT".to_owned(),
            depth: 10,
        }),
    )
    .await
    .expect("snapshot request writes");
    let response = read_response(&mut stream).await;

    match response {
        EngineResponse::Ack(ack) => match ack.result {
            AckResult::Snapshot(snapshot) => {
                assert_eq!(
                    snapshot.bids,
                    vec![(Decimal::new(100, 0), Decimal::new(2, 0))]
                );
                assert!(snapshot.asks.is_empty());
            }
            other => panic!("expected snapshot ack, got {other:?}"),
        },
        other => panic!("expected ack response, got {other:?}"),
    }
    server.shutdown().await;
}

#[tokio::test]
async fn cancel_removes_resting_order_from_snapshot() {
    let server = start_server().await;
    let mut stream = TcpStream::connect(server.addr)
        .await
        .expect("client connects");
    let order_id = Uuid::new_v4();

    send_place_limit(&mut stream, order_id)
        .await
        .expect("place request writes");
    let _place_response = read_response(&mut stream).await;

    write_json_frame(
        &mut stream,
        &EngineRequest::Cancel(CancelRequest {
            request_id: Uuid::new_v4(),
            symbol: "BTCUSDT".to_owned(),
            order_id,
        }),
    )
    .await
    .expect("cancel request writes");
    let response = read_response(&mut stream).await;

    match response {
        EngineResponse::Ack(ack) => match ack.result {
            AckResult::Cancel(cancel) => {
                assert_eq!(cancel.order_id, order_id);
                assert_eq!(cancel.status, OrderStatus::Canceled);
            }
            other => panic!("expected cancel ack, got {other:?}"),
        },
        other => panic!("expected ack response, got {other:?}"),
    }

    write_json_frame(
        &mut stream,
        &EngineRequest::Snapshot(SnapshotRequest {
            request_id: Uuid::new_v4(),
            symbol: "BTCUSDT".to_owned(),
            depth: 10,
        }),
    )
    .await
    .expect("snapshot request writes");
    let response = read_response(&mut stream).await;

    match response {
        EngineResponse::Ack(ack) => match ack.result {
            AckResult::Snapshot(snapshot) => {
                assert!(snapshot.bids.is_empty());
                assert!(snapshot.asks.is_empty());
            }
            other => panic!("expected snapshot ack, got {other:?}"),
        },
        other => panic!("expected ack response, got {other:?}"),
    }
    server.shutdown().await;
}

async fn start_server() -> RunningServer {
    let dir = std::env::temp_dir().join(format!("aether-cex-server-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("test directory created");
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("listener addr exists");
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

    RunningServer {
        addr,
        dir,
        handle,
        token,
    }
}

async fn send_place_limit(
    stream: &mut TcpStream,
    order_id: Uuid,
) -> Result<(), cex_proto::FrameError> {
    let order = EngineOrder {
        id: order_id,
        user_id: Uuid::new_v4(),
        symbol: "BTCUSDT".to_owned(),
        side: OrderSide::Buy,
        order_type: EngineOrderType::Limit,
        price: Some(Decimal::new(100, 0)),
        quantity: Decimal::new(2, 0),
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    };
    write_json_frame(
        stream,
        &EngineRequest::Place(PlaceRequest {
            request_id: Uuid::new_v4(),
            order,
        }),
    )
    .await
}

async fn read_response(stream: &mut TcpStream) -> EngineResponse {
    loop {
        let response: EngineResponse = read_json_frame(stream)
            .await
            .expect("response frame reads")
            .expect("response frame exists");
        if !matches!(response, EngineResponse::Event(_)) {
            return response;
        }
    }
}
