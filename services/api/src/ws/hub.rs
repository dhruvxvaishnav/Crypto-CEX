use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use cex_proto::{BookDeltaEvent, EngineEvent, SequencedEngineEvent, TradeFill};
use dashmap::DashMap;
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::channels;
use crate::repositories::markets as market_repo;

/// Broadcast channel capacity per channel key.
///
/// 256 events keeps memory bounded (~256 × ~512 B ≈ 128 KB per channel) while
/// giving slow consumers a comfortable buffer before they lag.
const CHANNEL_CAPACITY: usize = 256;

/// How long to retain diff frames in the per-symbol replay buffer.
const REPLAY_WINDOW: Duration = Duration::from_secs(60);

/// Serialised WebSocket frame, shared by reference across all subscribers.
pub type WsFrame = Arc<Bytes>;

/// Replay buffer entry: (captured_at, frame).
type ReplayEntry = (Instant, WsFrame);

/// Lightweight handle to the WebSocket subscription hub.
///
/// Clone freely — all clones share the same underlying `Arc`-backed maps.
/// Publishing serialises the JSON payload **once** per event; every subscriber
/// receives an `Arc<Bytes>` pointer (zero-copy fan-out).
#[derive(Clone)]
pub struct Hub {
    /// Per-channel broadcast senders keyed by channel name.
    senders: Arc<DashMap<Box<str>, broadcast::Sender<WsFrame>>>,
    /// Per-symbol diff replay buffer (last 60 s).
    replay: Arc<DashMap<Box<str>, Vec<ReplayEntry>>>,
}

impl Hub {
    /// Creates an empty hub.
    #[must_use]
    pub fn new() -> Self {
        Self {
            senders: Arc::new(DashMap::new()),
            replay: Arc::new(DashMap::new()),
        }
    }

    /// Returns a receiver for `channel`, creating the sender on first call.
    pub fn subscribe(&self, channel: &str) -> broadcast::Receiver<WsFrame> {
        self.senders
            .entry(channel.into())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Publishes `frame` to `channel`.
    ///
    /// Subscribers that have lagged more than `CHANNEL_CAPACITY` events will
    /// receive a `RecvError::Lagged` on their next recv — they should resync.
    pub fn publish(&self, channel: &str, frame: WsFrame) {
        if let Some(sender) = self.senders.get(channel) {
            // ignore SendError: no receivers is fine
            let _ = sender.send(frame);
        }
    }

    /// Publishes `frame` to `channel` AND appends to the diff replay buffer.
    fn publish_with_replay(&self, channel: &str, frame: WsFrame) {
        self.publish(channel, Arc::clone(&frame));
        let now = Instant::now();
        let mut entry = self.replay.entry(channel.into()).or_default();
        entry.retain(|(ts, _)| now.duration_since(*ts) < REPLAY_WINDOW);
        entry.push((now, frame));
    }

    /// Returns buffered diff frames after `after_seq` for `symbol`, or `None`
    /// if the gap is too large (client must resnapshot).
    ///
    /// We use `after_seq` only as a count guard here (PRD §11.5): if the
    /// buffer has fewer frames than needed the client must resnapshot.
    #[must_use]
    pub fn replay_diffs(&self, symbol: &str, after_seq: u64) -> Option<Vec<WsFrame>> {
        let channel = channels::book_diff(symbol);
        let entry = self.replay.get(channel.as_str())?;
        let now = Instant::now();
        let recent: Vec<WsFrame> = entry
            .iter()
            .filter(|(ts, _)| now.duration_since(*ts) < REPLAY_WINDOW)
            .map(|(_, f)| Arc::clone(f))
            .collect();
        // If after_seq is beyond our buffer we can't help.
        // usize::try_from is fallible on 32-bit; treat overflow as gap-too-large.
        let seq_idx = usize::try_from(after_seq).unwrap_or(usize::MAX);
        if seq_idx > recent.len() {
            return None;
        }
        Some(recent)
    }

    /// Processes engine events and fans them out to subscribers.
    ///
    /// Spawn this on a dedicated task — it blocks until the sender is dropped.
    pub async fn run(self, mut rx: tokio::sync::mpsc::Receiver<SequencedEngineEvent>) {
        while let Some(seq_event) = rx.recv().await {
            self.dispatch(seq_event);
        }
    }

    /// Drives the 1-second ticker publisher.
    ///
    /// Queries 24h rolling stats from the klines table every second and
    /// publishes to `ticker.<symbol>` and `ticker.all`.
    /// Spawn on a dedicated task alongside `run`.
    pub async fn run_ticker(self, pool: PgPool) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            match market_repo::ticker_snapshots(&pool).await {
                Ok(snapshots) => self.publish_ticker(&snapshots),
                Err(err) => {
                    tracing::warn!(err = %err, "ws.ticker.db_error");
                }
            }
        }
    }

    fn publish_ticker(&self, snapshots: &[market_repo::TickerSnapshot]) {
        let mut all_items = Vec::with_capacity(snapshots.len());
        for snap in snapshots {
            let price_change_pct = match (snap.last_price, snap.open_24h) {
                (Some(last), Some(open)) if !open.is_zero() => {
                    Some(((last - open) / open * rust_decimal::Decimal::ONE_HUNDRED).round_dp(4))
                }
                _ => None,
            };
            let item = serde_json::json!({
                "symbol":          snap.symbol,
                "lastPrice":       snap.last_price.map(|d| d.to_string()),
                "open24h":         snap.open_24h.map(|d| d.to_string()),
                "high24h":         snap.high_24h.map(|d| d.to_string()),
                "low24h":          snap.low_24h.map(|d| d.to_string()),
                "volume24h":       snap.volume_24h.map(|d| d.to_string()),
                "priceChangePct":  price_change_pct.map(|d| d.to_string()),
            });
            let channel = channels::ticker(&snap.symbol);
            let frame = make_frame(serde_json::json!({
                "channel": channel,
                "data": &item,
            }));
            self.publish(&channel, frame);
            all_items.push(item);
        }

        if !all_items.is_empty() {
            let all_frame = make_frame(serde_json::json!({
                "channel": channels::TICKER_ALL,
                "data": all_items,
            }));
            self.publish(channels::TICKER_ALL, all_frame);
        }
    }

    fn dispatch(&self, seq_event: SequencedEngineEvent) {
        let SequencedEngineEvent { seq, event } = seq_event;
        match event {
            EngineEvent::BookDelta(delta) => self.on_book_delta(delta, seq),
            EngineEvent::Fill(fill) => self.on_fill(fill, seq),
            EngineEvent::OrderAccepted(e) => self.on_order_accepted_event(e, seq),
            EngineEvent::OrderCanceled(e) => self.on_order_canceled_event(e, seq),
            EngineEvent::OrderRested(_) | EngineEvent::MarketStatus(_) => {}
        }
    }

    fn on_book_delta(&self, delta: BookDeltaEvent, seq: u64) {
        let channel = channels::book_diff(&delta.symbol);
        let frame = make_frame(serde_json::json!({
            "channel": channel,
            "seq": seq,
            "data": {
                "bids": price_levels(&delta.bids),
                "asks": price_levels(&delta.asks),
                "seq": seq,
            }
        }));
        self.publish_with_replay(&channel, frame);
    }

    fn on_fill(&self, fill: TradeFill, seq: u64) {
        let trade_channel = channels::trade(&fill.symbol);
        let frame = make_frame(serde_json::json!({
            "channel": trade_channel,
            "seq": seq,
            "data": {
                "id":    fill.trade_id,
                "price": fill.price.to_string(),
                "qty":   fill.quantity.to_string(),
                "side":  fill.taker_side,
                "ts":    fill.ts,
            }
        }));
        self.publish(&trade_channel, Arc::clone(&frame));

        // Private user.fills for both taker and maker sides.
        let user_fill_frame = make_frame(serde_json::json!({
            "channel": channels::USER_FILLS,
            "seq": seq,
            "data": {
                "tradeId":      fill.trade_id,
                "symbol":       fill.symbol,
                "takerOrderId": fill.taker_order_id,
                "makerOrderId": fill.maker_order_id,
                "price":        fill.price.to_string(),
                "qty":          fill.quantity.to_string(),
                "takerSide":    fill.taker_side,
                "ts":           fill.ts,
            }
        }));
        self.publish(channels::USER_FILLS, user_fill_frame);
    }

    fn on_order_accepted_event(
        &self,
        event: cex_proto::OrderAcceptedEvent,
        seq: u64,
    ) {
        let frame = make_frame(serde_json::json!({
            "channel": channels::USER_ORDERS,
            "seq": seq,
            "data": { "orderId": event.order.id, "status": "new" }
        }));
        self.publish(channels::USER_ORDERS, frame);
    }

    fn on_order_canceled_event(
        &self,
        event: cex_proto::OrderCanceledEvent,
        seq: u64,
    ) {
        let frame = make_frame(serde_json::json!({
            "channel": channels::USER_ORDERS,
            "seq": seq,
            "data": { "orderId": event.order_id, "status": "canceled", "reason": event.reason }
        }));
        self.publish(channels::USER_ORDERS, frame);
    }

    /// Publishes a balance update to the `user.balances` private channel.
    pub fn publish_balance_update(&self, user_id: Uuid, asset: &str, available: &str, locked: &str, seq: u64) {
        let frame = make_frame(serde_json::json!({
            "channel": channels::USER_BALANCES,
            "seq": seq,
            "data": {
                "userId": user_id,
                "asset": asset,
                "available": available,
                "locked": locked,
            }
        }));
        self.publish(channels::USER_BALANCES, frame);
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

fn make_frame(value: serde_json::Value) -> WsFrame {
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    Arc::new(Bytes::from(bytes))
}

fn price_levels(levels: &[(rust_decimal::Decimal, rust_decimal::Decimal)]) -> Vec<[String; 2]> {
    levels
        .iter()
        .map(|(p, q)| [p.to_string(), q.to_string()])
        .collect()
}
