use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use cex_proto::{
    read_json_frame, AckResponse, AckResult, BookSnapshot, CancelAck, CancelAllAck,
    CancelAllRequest, CancelRequest, EngineRequest, EngineResponse, MarketControlRequest,
    MarketStatusAck, OrderAck, PingRequest, PlaceRequest, RejectResponse, SequencedEngineEvent,
    SnapshotRequest,
};
use dashmap::DashMap;
use opentelemetry::propagation::Injector;
use thiserror::Error;
use tokio::io::AsyncWriteExt as _;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use uuid::Uuid;

/// Errors returned by [`EngineClient`] operations.
#[derive(Debug, Error)]
pub enum EngineClientError {
    /// Connection or frame IO failed.
    #[error("engine io failed")]
    Io,
    /// Request could not be serialised.
    #[error("engine request serialisation failed")]
    Serialization,
    /// Request timed out waiting for an engine response.
    #[error("engine timed out")]
    Timeout,
    /// Engine returned an unexpected response variant.
    #[error("unexpected engine response")]
    Unexpected,
    /// Engine explicitly rejected the request.
    #[error("engine rejected: {code}")]
    Rejected {
        /// Stable error code from the engine.
        code: String,
        /// Human-readable message.
        message: String,
    },
}

type PendingMap = Arc<DashMap<Uuid, oneshot::Sender<EngineResponse>>>;

/// Persistent, multiplexed TCP client for the matching engine.
///
/// A single `TcpStream` is shared across all concurrent requests. The write
/// half is protected by a `Mutex` held only for the frame-write bytes — not
/// while awaiting the response — so concurrent orders overlap freely. The read
/// half runs in a background task that dispatches responses to waiting callers
/// via per-request `oneshot` channels keyed by `requestId`. If the connection
/// drops, the next call transparently reconnects.
///
/// Every outbound frame carries the W3C `traceparent` field so that the engine
/// spans can be linked to the API parent trace (PRD §19.3).
#[derive(Clone)]
pub struct EngineClient {
    addr: SocketAddr,
    timeout: Duration,
    /// `None` means currently disconnected.
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    pending: PendingMap,
    /// Forwards unsolicited engine events to the WS hub.
    event_tx: Option<tokio::sync::mpsc::Sender<SequencedEngineEvent>>,
}

impl EngineClient {
    /// Creates a client that connects lazily on first use.
    #[must_use]
    pub fn new(addr: SocketAddr, timeout: Duration) -> Self {
        Self {
            addr,
            timeout,
            writer: Arc::new(Mutex::new(None)),
            pending: Arc::new(DashMap::new()),
            event_tx: None,
        }
    }

    /// Attaches a channel that receives unsolicited engine events for the WS hub.
    #[must_use]
    pub fn with_event_tx(mut self, tx: tokio::sync::mpsc::Sender<SequencedEngineEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Pings the engine; used by the readiness check.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] when the engine cannot be reached, times
    /// out, or responds incorrectly.
    pub async fn ping(&self, request_id: Uuid) -> Result<(), EngineClientError> {
        match self
            .send(EngineRequest::Ping(PingRequest { request_id }), request_id)
            .await?
        {
            EngineResponse::Pong(p) if p.request_id == request_id => Ok(()),
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Places an order; returns the engine ack on success.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] on IO failure, timeout, or engine reject.
    pub async fn place(&self, req: PlaceRequest) -> Result<OrderAck, EngineClientError> {
        let request_id = req.request_id;
        match self.send(EngineRequest::Place(req), request_id).await? {
            EngineResponse::Ack(AckResponse {
                result: AckResult::Order(ack),
                ..
            }) => Ok(ack),
            EngineResponse::Reject(RejectResponse { code, message, .. }) => {
                Err(EngineClientError::Rejected { code, message })
            }
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Cancels a single order; returns the engine ack on success.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] on IO failure, timeout, or engine reject.
    pub async fn cancel(&self, req: CancelRequest) -> Result<CancelAck, EngineClientError> {
        let request_id = req.request_id;
        match self.send(EngineRequest::Cancel(req), request_id).await? {
            EngineResponse::Ack(AckResponse {
                result: AckResult::Cancel(ack),
                ..
            }) => Ok(ack),
            EngineResponse::Reject(RejectResponse { code, message, .. }) => {
                Err(EngineClientError::Rejected { code, message })
            }
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Cancels all open orders for a user, optionally scoped to one symbol.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] on IO failure, timeout, or engine reject.
    pub async fn cancel_all(
        &self,
        req: CancelAllRequest,
    ) -> Result<CancelAllAck, EngineClientError> {
        let request_id = req.request_id;
        match self.send(EngineRequest::CancelAll(req), request_id).await? {
            EngineResponse::Ack(AckResponse {
                result: AckResult::CancelAll(ack),
                ..
            }) => Ok(ack),
            EngineResponse::Reject(RejectResponse { code, message, .. }) => {
                Err(EngineClientError::Rejected { code, message })
            }
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Halts a market; new orders for the symbol are rejected by the engine.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] on IO failure, timeout, or engine reject.
    pub async fn halt_market(
        &self,
        req: MarketControlRequest,
    ) -> Result<MarketStatusAck, EngineClientError> {
        let request_id = req.request_id;
        match self
            .send(EngineRequest::HaltMarket(req), request_id)
            .await?
        {
            EngineResponse::Ack(AckResponse {
                result: AckResult::MarketStatus(ack),
                ..
            }) => Ok(ack),
            EngineResponse::Reject(RejectResponse { code, message, .. }) => {
                Err(EngineClientError::Rejected { code, message })
            }
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Resumes a halted market.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] on IO failure, timeout, or engine reject.
    pub async fn resume_market(
        &self,
        req: MarketControlRequest,
    ) -> Result<MarketStatusAck, EngineClientError> {
        let request_id = req.request_id;
        match self
            .send(EngineRequest::ResumeMarket(req), request_id)
            .await?
        {
            EngineResponse::Ack(AckResponse {
                result: AckResult::MarketStatus(ack),
                ..
            }) => Ok(ack),
            EngineResponse::Reject(RejectResponse { code, message, .. }) => {
                Err(EngineClientError::Rejected { code, message })
            }
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Fetches an L2 order-book snapshot from the engine.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] on IO failure, timeout, or engine reject.
    pub async fn snapshot(&self, req: SnapshotRequest) -> Result<BookSnapshot, EngineClientError> {
        let request_id = req.request_id;
        match self.send(EngineRequest::Snapshot(req), request_id).await? {
            EngineResponse::Ack(AckResponse {
                result: AckResult::Snapshot(snap),
                ..
            }) => Ok(snap),
            _ => Err(EngineClientError::Unexpected),
        }
    }

    // ── Internal ─────────────────────────────────────────────────────────

    /// Sends a request and awaits its response by `requestId`.
    ///
    /// The write-mutex is held only for the frame bytes; concurrent requests
    /// wait in parallel on their own `oneshot` channels after the write.
    async fn send(
        &self,
        request: EngineRequest,
        request_id: Uuid,
    ) -> Result<EngineResponse, EngineClientError> {
        let (tx, rx) = oneshot::channel();
        self.pending.insert(request_id, tx);

        // Capture the W3C traceparent from the current tracing span before
        // acquiring the write lock to keep the lock hold time minimal.
        let traceparent = extract_traceparent();

        let payload = serialize_with_traceparent(&request, traceparent).map_err(|_| {
            self.pending.remove(&request_id);
            EngineClientError::Serialization
        })?;

        let write_ok = {
            let mut guard = self.writer.lock().await;
            self.write_or_reconnect(&mut guard, &payload).await
        };

        if let Err(e) = write_ok {
            self.pending.remove(&request_id);
            return Err(e);
        }

        timeout(self.timeout, rx)
            .await
            .map_err(|_| {
                self.pending.remove(&request_id);
                EngineClientError::Timeout
            })?
            .map_err(|_| EngineClientError::Io)
    }

    /// Writes `payload` bytes to the engine, reconnecting first if needed.
    ///
    /// # Panics
    ///
    /// Does not panic.
    async fn write_or_reconnect(
        &self,
        guard: &mut Option<OwnedWriteHalf>,
        payload: &[u8],
    ) -> Result<(), EngineClientError> {
        if let Some(writer) = guard.as_mut() {
            if write_framed(writer, payload).await.is_ok() {
                return Ok(());
            }
            *guard = None;
        }

        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|_| EngineClientError::Io)?;
        let (read_half, mut write_half) = stream.into_split();

        write_framed(&mut write_half, payload)
            .await
            .map_err(|_| EngineClientError::Io)?;

        let pending = Arc::clone(&self.pending);
        let writer_arc = Arc::clone(&self.writer);
        let event_tx = self.event_tx.clone();
        tokio::spawn(reader_loop(read_half, pending, event_tx, writer_arc));

        *guard = Some(write_half);
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Adapter so `TextMapPropagator::inject_context` can write into a [`HashMap`].
struct HashMapCarrier(HashMap<String, String>);

impl Injector for HashMapCarrier {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }
}

/// Extracts the W3C `traceparent` string from the currently-active tracing span.
fn extract_traceparent() -> Option<String> {
    let ctx = tracing::Span::current().context();
    let mut carrier = HashMapCarrier(HashMap::new());
    opentelemetry::global::get_text_map_propagator(|prop| {
        prop.inject_context(&ctx, &mut carrier);
    });
    carrier.0.remove("traceparent")
}

/// Serialises the request to JSON and injects an optional `traceparent` field.
///
/// The engine server ignores unknown top-level fields (serde default), so the
/// extra field propagates safely without breaking the engine wire protocol.
fn serialize_with_traceparent(
    request: &EngineRequest,
    traceparent: Option<String>,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut value = serde_json::to_value(request)?;
    if let (Some(tp), Some(map)) = (traceparent, value.as_object_mut()) {
        map.insert("traceparent".to_owned(), serde_json::Value::String(tp));
    }
    serde_json::to_vec(&value)
}

/// Writes a length-prefixed frame to `writer`.
async fn write_framed(writer: &mut OwnedWriteHalf, payload: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"))?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await
}

/// Reads engine responses and dispatches them to waiting callers.
///
/// On IO error or clean EOF, marks the write-half `None` so the next send
/// reconnects, and drops all pending senders so callers get `Err` quickly.
async fn reader_loop(
    mut read_half: OwnedReadHalf,
    pending: PendingMap,
    event_tx: Option<tokio::sync::mpsc::Sender<SequencedEngineEvent>>,
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
) {
    loop {
        match read_json_frame::<_, EngineResponse>(&mut read_half).await {
            Ok(Some(EngineResponse::Event(seq_event))) => {
                if let Some(tx) = &event_tx {
                    // Non-blocking: drop event if hub buffer is full rather than
                    // back-pressuring the engine reader.
                    let _ = tx.try_send(seq_event);
                }
            }
            Ok(Some(response)) => {
                if let Some(id) = response_id(&response) {
                    if let Some((_, sender)) = pending.remove(&id) {
                        let _ = sender.send(response);
                    }
                }
            }
            Ok(None) | Err(_) => break,
        }
    }

    tracing::warn!(event = "engine.tcp.disconnected");
    *writer.lock().await = None;
    pending.clear();
}

const fn response_id(response: &EngineResponse) -> Option<Uuid> {
    match response {
        EngineResponse::Ack(r) => Some(r.request_id),
        EngineResponse::Reject(r) => Some(r.request_id),
        EngineResponse::Pong(r) => Some(r.request_id),
        EngineResponse::Event(_) => None,
    }
}
