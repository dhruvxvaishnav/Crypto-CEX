use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time::interval;
use uuid::Uuid;

use crate::state::AppState;
use crate::ws::channels;
use crate::ws::hub::WsFrame;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const PONG_DEADLINE: Duration = Duration::from_secs(30);

static LAG_FRAME: &[u8] =
    br#"{"error":{"code":"GAP_TOO_LARGE","message":"Client lagged; please resync"}}"#;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

/// `GET /ws[?token=<jwt>]` — WebSocket upgrade entry point.
///
/// # Errors
///
/// Never errors; failures are handled inside the spawned connection task.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let authenticated_user_id = q.token.as_deref().and_then(|token| {
        state
            .token_config
            .decode_access_token(token)
            .map(|claims| claims.sub)
            .ok()
    });

    ws.on_upgrade(move |socket| connection(socket, state, authenticated_user_id))
}

// ── Per-connection task ───────────────────────────────────────────────────────

/// Drives a single WebSocket connection.
///
/// All outbound frames (acks, broadcast events, heartbeat pings) flow through
/// an unbounded mpsc channel so `socket` is borrowed in only one `select!` arm.
async fn connection(mut socket: WebSocket, state: AppState, user_id: Option<Uuid>) {
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<WsFrame>();

    let mut subscriptions: HashSet<String> = HashSet::new();
    let mut forwarder_handles: Vec<JoinHandle<()>> = Vec::new();
    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    let mut awaiting_pong = false;

    loop {
        tokio::select! {
            // ── Inbound client frame ──────────────────────────────────────
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(
                            text.as_str(),
                            &out_tx,
                            &state,
                            &mut subscriptions,
                            &mut forwarder_handles,
                            user_id,
                        )
                        .await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        awaiting_pong = false;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            // ── Outbound: acks, broadcast events, etc. ────────────────────
            frame = out_rx.recv() => {
                match frame {
                    Some(f) => {
                        let text = String::from_utf8_lossy(&f).into_owned();
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }

            // ── Server-side heartbeat ─────────────────────────────────────
            _ = heartbeat.tick() => {
                if awaiting_pong {
                    tracing::warn!(event = "ws.client_timeout");
                    break;
                }
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
                awaiting_pong = true;
                heartbeat = interval(PONG_DEADLINE);
            }
        }
    }

    for handle in forwarder_handles {
        handle.abort();
    }
}

// ── Broadcast forwarder ───────────────────────────────────────────────────────

/// Spawns a task that forwards from a broadcast receiver into the connection's
/// outbound channel. On lag, sends a `GAP_TOO_LARGE` error frame.
fn spawn_forwarder(
    mut rx: broadcast::Receiver<WsFrame>,
    out_tx: UnboundedSender<WsFrame>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(frame) => {
                    if out_tx.send(frame).is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let lag = Arc::new(Bytes::from_static(LAG_FRAME));
                    if out_tx.send(lag).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

// ── Client message handling ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ClientMessage {
    id: Option<String>,
    method: String,
    params: Option<ClientParams>,
}

#[derive(Debug, Deserialize)]
struct ClientParams {
    channels: Option<Vec<String>>,
    #[serde(rename = "afterSeq")]
    after_seq: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ServerAck {
    id: Option<String>,
    result: Value,
}

#[derive(Debug, Serialize)]
struct ServerError {
    id: Option<String>,
    error: ErrorPayload,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    code: String,
    message: String,
}

async fn handle_client_message(
    raw: &str,
    out_tx: &UnboundedSender<WsFrame>,
    state: &AppState,
    subscriptions: &mut HashSet<String>,
    forwarder_handles: &mut Vec<JoinHandle<()>>,
    user_id: Option<Uuid>,
) {
    let msg: ClientMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(_) => {
            send_error_to_tx(out_tx, None, "INVALID_MESSAGE", "Invalid JSON message");
            return;
        }
    };

    match msg.method.as_str() {
        "ping" => {
            let ack = ServerAck {
                id: msg.id,
                result: serde_json::json!({ "ts": chrono_now_ms() }),
            };
            send_to_tx(out_tx, &ack);
        }

        "subscribe" => {
            let channels_req = msg
                .params
                .as_ref()
                .and_then(|p| p.channels.as_ref())
                .cloned()
                .unwrap_or_default();
            let after_seq = msg.params.as_ref().and_then(|p| p.after_seq);

            let mut subscribed = Vec::new();
            for channel in channels_req {
                if is_private_channel(&channel) && user_id.is_none() {
                    send_error_to_tx(
                        out_tx,
                        msg.id.clone(),
                        "FORBIDDEN",
                        "Token required for private channels",
                    );
                    continue;
                }

                if subscriptions.contains(&channel) {
                    subscribed.push(channel.clone());
                    continue;
                }

                // Snapshot + diff: push snapshot before joining the diff stream.
                if let Some(symbol) = book_diff_symbol(&channel) {
                    send_book_snapshot(out_tx, state, symbol, after_seq).await;
                }

                let rx = state.hub.subscribe(&channel);
                forwarder_handles.push(spawn_forwarder(rx, out_tx.clone()));
                subscriptions.insert(channel.clone());
                subscribed.push(channel);
            }

            send_to_tx(
                out_tx,
                &ServerAck {
                    id: msg.id,
                    result: serde_json::json!({ "channels": subscribed }),
                },
            );
        }

        "unsubscribe" => {
            let channels_req = msg
                .params
                .as_ref()
                .and_then(|p| p.channels.as_ref())
                .cloned()
                .unwrap_or_default();
            for ch in &channels_req {
                subscriptions.remove(ch);
                // Forwarder tasks run until their receiver is dropped or out_tx
                // closes. On unsubscribe we just stop listening; the forwarder
                // will exit on the next closed-channel error.
            }
            send_to_tx(
                out_tx,
                &ServerAck {
                    id: msg.id,
                    result: serde_json::json!({ "channels": channels_req }),
                },
            );
        }

        other => {
            send_error_to_tx(
                out_tx,
                msg.id,
                "UNKNOWN_METHOD",
                &format!("Unknown method: {other}"),
            );
        }
    }
}

async fn send_book_snapshot(
    out_tx: &UnboundedSender<WsFrame>,
    state: &AppState,
    symbol: &str,
    after_seq: Option<u64>,
) {
    // If the client has a recent seq, try to replay buffered diffs first.
    if let Some(seq) = after_seq {
        if let Some(frames) = state.hub.replay_diffs(symbol, seq) {
            for frame in frames {
                let _ = out_tx.send(frame);
            }
            return;
        }
        let err = Arc::new(Bytes::from_static(
            br#"{"error":{"code":"GAP_TOO_LARGE","message":"Gap too large; sending full snapshot"}}"#,
        ));
        let _ = out_tx.send(err);
    }

    // Full snapshot from engine.
    let snap = match state
        .engine
        .snapshot(cex_proto::SnapshotRequest {
            request_id: Uuid::new_v4(),
            symbol: symbol.to_uppercase(),
            depth: 100,
        })
        .await
    {
        Ok(s) => s,
        Err(_) => return,
    };

    let bids: Vec<[String; 2]> = snap
        .bids
        .iter()
        .map(|(p, q)| [p.to_string(), q.to_string()])
        .collect();
    let asks: Vec<[String; 2]> = snap
        .asks
        .iter()
        .map(|(p, q)| [p.to_string(), q.to_string()])
        .collect();

    let payload = serde_json::json!({
        "channel": format!("book.{symbol}.snapshot"),
        "seq": snap.seq,
        "data": { "bids": bids, "asks": asks, "seq": snap.seq }
    });
    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    let _ = out_tx.send(Arc::new(Bytes::from(bytes)));
}

fn is_private_channel(channel: &str) -> bool {
    matches!(
        channel,
        channels::USER_ORDERS | channels::USER_FILLS | channels::USER_BALANCES
    )
}

fn book_diff_symbol(channel: &str) -> Option<&str> {
    channel
        .strip_prefix("book.")
        .and_then(|s| s.strip_suffix(".diff"))
}

fn send_to_tx<T: Serialize>(out_tx: &UnboundedSender<WsFrame>, value: &T) {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let _ = out_tx.send(Arc::new(Bytes::from(bytes)));
}

fn send_error_to_tx(out_tx: &UnboundedSender<WsFrame>, id: Option<String>, code: &str, message: &str) {
    let msg = ServerError {
        id,
        error: ErrorPayload {
            code: code.to_owned(),
            message: message.to_owned(),
        },
    };
    send_to_tx(out_tx, &msg);
}

fn chrono_now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
