use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use cex_core::book::{EngineEvent as CoreEngineEvent, OrderBook};
use cex_core::types::{Fill as CoreFill, Order};
use cex_core::wal::{Wal, WalCommand};
use cex_proto::{
    AckResponse, AckResult, BookDeltaEvent, BookSnapshot, CancelAck, CancelAllAck, CancelRequest,
    EngineEvent, EngineOrder, EngineRequest, EngineResponse, MarketControlRequest, MarketStatus,
    MarketStatusAck, MarketStatusEvent, OrderAcceptedEvent, OrderAck, OrderCanceledEvent,
    OrderRestedEvent, OrderStatus, PingRequest, PongResponse, PriceLevel, SequencedEngineEvent,
    SnapshotRequest, TradeFill,
};
use rust_decimal::Decimal;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::config::EngineServerConfig;
use crate::error::ServerError;
use crate::event_log::EventLog;
use crate::time::{Clock, IdSource};
use crate::wire::{
    cancelled_order_ids, immediate_pre_reject, last_seq_or_current, market_halted, order_not_found,
    order_status_for_place, to_core_order, to_wire_cancel_reason, to_wire_side,
    validate_wire_order,
};

#[derive(Debug, Clone)]
struct OpenOrder {
    user_id: Uuid,
    symbol: String,
    remaining: Decimal,
}

pub(crate) struct ResponseBundle {
    pub(crate) response: EngineResponse,
    pub(crate) events: Vec<SequencedEngineEvent>,
}

pub(crate) struct EngineState {
    books: HashMap<String, OrderBook>,
    command_wal: Wal,
    event_log: EventLog,
    event_tx: broadcast::Sender<SequencedEngineEvent>,
    halted_symbols: HashSet<String>,
    open_orders: HashMap<Uuid, OpenOrder>,
    id_source: Arc<dyn IdSource>,
    clock: Arc<dyn Clock>,
    snapshot_depth: usize,
}

impl EngineState {
    pub(crate) fn open(
        config: &EngineServerConfig,
        event_tx: broadcast::Sender<SequencedEngineEvent>,
        id_source: Arc<dyn IdSource>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, ServerError> {
        let (books, open_orders) = replay_books(&config.command_wal_path)?;
        let command_wal = Wal::open(&config.command_wal_path)?;
        let event_log = EventLog::open(&config.event_log_path)?;
        Ok(Self {
            books,
            command_wal,
            event_log,
            event_tx,
            halted_symbols: HashSet::new(),
            open_orders,
            id_source,
            clock,
            snapshot_depth: config.snapshot_depth,
        })
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<SequencedEngineEvent> {
        self.event_tx.subscribe()
    }

    pub(crate) fn broadcast(&self, event: SequencedEngineEvent) {
        if let Err(error) = self.event_tx.send(event) {
            tracing::debug!(
                skipped_seq = error.0.seq,
                "engine.event.broadcast.no_receivers"
            );
        }
    }

    pub(crate) fn handle_request(
        &mut self,
        request: EngineRequest,
    ) -> Result<ResponseBundle, ServerError> {
        match request {
            EngineRequest::Place(request) => self.place_order(request.request_id, request.order),
            EngineRequest::Cancel(request) => self.cancel_order(request),
            EngineRequest::CancelAll(request) => {
                self.cancel_all(request.request_id, request.user_id, request.symbol)
            }
            EngineRequest::HaltMarket(request) => self.halt_market(request),
            EngineRequest::ResumeMarket(request) => self.resume_market(request),
            EngineRequest::Snapshot(request) => Ok(self.snapshot(request)),
            EngineRequest::Ping(request) => Ok(Self::pong(request)),
        }
    }

    fn place_order(
        &mut self,
        request_id: Uuid,
        wire_order: EngineOrder,
    ) -> Result<ResponseBundle, ServerError> {
        if let Some(reject) = validate_wire_order(request_id, &wire_order) {
            return Ok(ResponseBundle {
                response: EngineResponse::Reject(reject),
                events: Vec::new(),
            });
        }

        let symbol = wire_order.symbol.clone();
        if self.halted_symbols.contains(&symbol) {
            return Ok(market_halted(request_id));
        }

        let order = to_core_order(&wire_order);
        self.command_wal
            .append(WalCommand::PlaceOrder(order.clone()))?;

        let core_events = {
            let book = self.books.entry(symbol.clone()).or_default();
            if let Some(linked_id) = order.oco_linked_id {
                book.register_oco(order.id, linked_id);
            }
            book.match_order(order.clone())
        };

        if let Some(rejected) = immediate_pre_reject(request_id, order.id, &core_events) {
            return Ok(ResponseBundle {
                response: EngineResponse::Reject(rejected),
                events: Vec::new(),
            });
        }

        apply_open_order_updates(&mut self.open_orders, Some(&order), &core_events);
        let (mut wire_events, fills) = self.to_wire_core_events(&symbol, &core_events);
        wire_events.insert(
            0,
            EngineEvent::OrderAccepted(OrderAcceptedEvent {
                order: wire_order.clone(),
            }),
        );
        wire_events.push(self.book_delta_event(&symbol));
        let events = self.sequence_events(wire_events)?;
        let seq = last_seq_or_current(&events, self.event_log.sequence());
        let status = order_status_for_place(order.id, order.quantity, &core_events);
        let result = AckResult::Order(OrderAck {
            order_id: order.id,
            status,
            fills: fills
                .into_iter()
                .filter(|fill| fill.taker_order_id == order.id)
                .collect(),
            seq,
        });

        Ok(ResponseBundle {
            response: EngineResponse::Ack(AckResponse { request_id, result }),
            events,
        })
    }

    fn cancel_order(&mut self, request: CancelRequest) -> Result<ResponseBundle, ServerError> {
        let Some(meta) = self.open_orders.get(&request.order_id).cloned() else {
            return Ok(order_not_found(request.request_id, "order was not found"));
        };
        if meta.symbol != request.symbol {
            return Ok(order_not_found(
                request.request_id,
                "order was not found for symbol",
            ));
        }

        let core_events = self.cancel_known_order(request.order_id)?;
        if core_events.is_empty() {
            return Ok(order_not_found(request.request_id, "order was not found"));
        }

        let mut wire_events = self.to_wire_cancel_events(&core_events);
        wire_events.push(self.book_delta_event(&request.symbol));
        let events = self.sequence_events(wire_events)?;
        let seq = last_seq_or_current(&events, self.event_log.sequence());
        let result = AckResult::Cancel(CancelAck {
            order_id: request.order_id,
            status: OrderStatus::Canceled,
            seq,
        });

        Ok(ResponseBundle {
            response: EngineResponse::Ack(AckResponse {
                request_id: request.request_id,
                result,
            }),
            events,
        })
    }

    fn cancel_all(
        &mut self,
        request_id: Uuid,
        user_id: Uuid,
        symbol: Option<String>,
    ) -> Result<ResponseBundle, ServerError> {
        let mut order_ids: Vec<Uuid> = self
            .open_orders
            .iter()
            .filter(|(_, meta)| meta.user_id == user_id)
            .filter(|(_, meta)| symbol.as_ref().map_or(true, |s| s == &meta.symbol))
            .map(|(order_id, _)| *order_id)
            .collect();
        order_ids.sort_unstable();

        let mut canceled_ids = Vec::with_capacity(order_ids.len());
        let mut changed_symbols = BTreeSet::new();
        let mut wire_events = Vec::new();
        for order_id in order_ids {
            let Some(meta) = self.open_orders.get(&order_id).cloned() else {
                continue;
            };
            let core_events = self.cancel_known_order(order_id)?;
            if core_events.is_empty() {
                continue;
            }
            changed_symbols.insert(meta.symbol);
            canceled_ids.extend(cancelled_order_ids(&core_events));
            wire_events.extend(self.to_wire_cancel_events(&core_events));
        }
        canceled_ids.sort_unstable();
        canceled_ids.dedup();
        for changed_symbol in changed_symbols {
            wire_events.push(self.book_delta_event(&changed_symbol));
        }

        let events = self.sequence_events(wire_events)?;
        let seq = last_seq_or_current(&events, self.event_log.sequence());
        let result = AckResult::CancelAll(CancelAllAck {
            order_ids: canceled_ids,
            seq,
        });

        Ok(ResponseBundle {
            response: EngineResponse::Ack(AckResponse { request_id, result }),
            events,
        })
    }

    fn halt_market(
        &mut self,
        request: MarketControlRequest,
    ) -> Result<ResponseBundle, ServerError> {
        self.halted_symbols.insert(request.symbol.clone());
        self.market_status_response(request, MarketStatus::Halted)
    }

    fn resume_market(
        &mut self,
        request: MarketControlRequest,
    ) -> Result<ResponseBundle, ServerError> {
        self.halted_symbols.remove(&request.symbol);
        self.market_status_response(request, MarketStatus::Trading)
    }

    fn market_status_response(
        &mut self,
        request: MarketControlRequest,
        status: MarketStatus,
    ) -> Result<ResponseBundle, ServerError> {
        let events = self.sequence_events(vec![EngineEvent::MarketStatus(MarketStatusEvent {
            symbol: request.symbol.clone(),
            status,
        })])?;
        let seq = last_seq_or_current(&events, self.event_log.sequence());
        let result = AckResult::MarketStatus(MarketStatusAck {
            symbol: request.symbol,
            status,
            seq,
        });

        Ok(ResponseBundle {
            response: EngineResponse::Ack(AckResponse {
                request_id: request.request_id,
                result,
            }),
            events,
        })
    }

    fn snapshot(&self, request: SnapshotRequest) -> ResponseBundle {
        let depth = request.depth.min(self.snapshot_depth);
        let snapshot = self.book_snapshot(&request.symbol, depth);
        ResponseBundle {
            response: EngineResponse::Ack(AckResponse {
                request_id: request.request_id,
                result: AckResult::Snapshot(snapshot),
            }),
            events: Vec::new(),
        }
    }

    fn pong(request: PingRequest) -> ResponseBundle {
        ResponseBundle {
            response: EngineResponse::Pong(PongResponse {
                request_id: request.request_id,
            }),
            events: Vec::new(),
        }
    }

    fn cancel_known_order(&mut self, order_id: Uuid) -> Result<Vec<CoreEngineEvent>, ServerError> {
        let Some(meta) = self.open_orders.get(&order_id).cloned() else {
            return Ok(Vec::new());
        };
        self.command_wal.append(WalCommand::CancelOrder(order_id))?;
        let Some(book) = self.books.get_mut(&meta.symbol) else {
            return Ok(Vec::new());
        };
        let core_events = book.cancel_order(order_id);
        apply_open_order_updates(&mut self.open_orders, None, &core_events);
        Ok(core_events)
    }

    fn sequence_events(
        &mut self,
        events: Vec<EngineEvent>,
    ) -> Result<Vec<SequencedEngineEvent>, ServerError> {
        events
            .into_iter()
            .map(|event| self.event_log.append_next(event).map_err(ServerError::from))
            .collect()
    }

    fn to_wire_core_events(
        &self,
        symbol: &str,
        events: &[CoreEngineEvent],
    ) -> (Vec<EngineEvent>, Vec<TradeFill>) {
        let mut wire_events = Vec::with_capacity(events.len());
        let mut fills = Vec::new();
        for event in events {
            match event {
                CoreEngineEvent::Fill(fill) => {
                    let wire_fill = self.to_wire_fill(symbol, fill);
                    fills.push(wire_fill.clone());
                    wire_events.push(EngineEvent::Fill(wire_fill));
                }
                CoreEngineEvent::Rested {
                    order_id,
                    remaining,
                } => wire_events.push(EngineEvent::OrderRested(OrderRestedEvent {
                    order_id: *order_id,
                    remaining: *remaining,
                })),
                CoreEngineEvent::Cancelled { order_id, reason } => {
                    wire_events.push(EngineEvent::OrderCanceled(OrderCanceledEvent {
                        order_id: *order_id,
                        reason: to_wire_cancel_reason(*reason),
                    }));
                }
                CoreEngineEvent::StopTriggered { .. } => {}
            }
        }
        (wire_events, fills)
    }

    fn to_wire_cancel_events(&self, events: &[CoreEngineEvent]) -> Vec<EngineEvent> {
        events
            .iter()
            .filter_map(|event| match event {
                CoreEngineEvent::Cancelled { order_id, reason } => {
                    Some(EngineEvent::OrderCanceled(OrderCanceledEvent {
                        order_id: *order_id,
                        reason: to_wire_cancel_reason(*reason),
                    }))
                }
                _ => None,
            })
            .collect()
    }

    fn to_wire_fill(&self, symbol: &str, fill: &CoreFill) -> TradeFill {
        TradeFill {
            trade_id: self.id_source.next_uuid(),
            symbol: symbol.to_owned(),
            taker_order_id: fill.taker_order_id,
            maker_order_id: fill.maker_order_id,
            price: fill.price,
            quantity: fill.quantity,
            taker_side: to_wire_side(fill.taker_side),
            ts: self.clock.now_rfc3339(),
        }
    }

    fn book_delta_event(&self, symbol: &str) -> EngineEvent {
        let snapshot = self.book_snapshot(symbol, self.snapshot_depth);
        EngineEvent::BookDelta(BookDeltaEvent {
            symbol: snapshot.symbol,
            bids: snapshot.bids,
            asks: snapshot.asks,
        })
    }

    fn book_snapshot(&self, symbol: &str, depth: usize) -> BookSnapshot {
        let (bids, asks) = self.books.get(symbol).map_or_else(
            || (Vec::new(), Vec::new()),
            |book| depth_levels(book, depth),
        );
        BookSnapshot {
            symbol: symbol.to_owned(),
            bids,
            asks,
            seq: self.event_log.sequence(),
        }
    }
}

fn replay_books(
    path: &std::path::Path,
) -> Result<(HashMap<String, OrderBook>, HashMap<Uuid, OpenOrder>), ServerError> {
    let entries = Wal::replay(path)?;
    let mut books: HashMap<String, OrderBook> = HashMap::new();
    let mut open_orders: HashMap<Uuid, OpenOrder> = HashMap::new();

    for entry in entries {
        match entry.command {
            WalCommand::PlaceOrder(order) => {
                let symbol = order.symbol.to_string();
                let book = books.entry(symbol).or_default();
                if let Some(linked_id) = order.oco_linked_id {
                    book.register_oco(order.id, linked_id);
                }
                let events = book.match_order(order.clone());
                apply_open_order_updates(&mut open_orders, Some(&order), &events);
            }
            WalCommand::CancelOrder(order_id) => {
                replay_cancel(&mut books, &mut open_orders, order_id);
            }
        }
    }

    Ok((books, open_orders))
}

fn replay_cancel(
    books: &mut HashMap<String, OrderBook>,
    open_orders: &mut HashMap<Uuid, OpenOrder>,
    order_id: Uuid,
) {
    let Some(meta) = open_orders.get(&order_id).cloned() else {
        return;
    };
    let Some(book) = books.get_mut(&meta.symbol) else {
        return;
    };
    let events = book.cancel_order(order_id);
    apply_open_order_updates(open_orders, None, &events);
}

fn apply_open_order_updates(
    open_orders: &mut HashMap<Uuid, OpenOrder>,
    submitted: Option<&Order>,
    events: &[CoreEngineEvent],
) {
    for event in events {
        match event {
            CoreEngineEvent::Fill(fill) => {
                decrement_open_order(open_orders, fill.maker_order_id, fill.quantity);
                decrement_open_order(open_orders, fill.taker_order_id, fill.quantity);
            }
            CoreEngineEvent::Rested {
                order_id,
                remaining,
            } => {
                if let Some(order) = submitted.filter(|order| order.id == *order_id) {
                    open_orders.insert(
                        *order_id,
                        OpenOrder {
                            user_id: order.user_id,
                            symbol: order.symbol.to_string(),
                            remaining: *remaining,
                        },
                    );
                } else if let Some(meta) = open_orders.get_mut(order_id) {
                    meta.remaining = *remaining;
                }
            }
            CoreEngineEvent::Cancelled { order_id, .. } => {
                open_orders.remove(order_id);
            }
            CoreEngineEvent::StopTriggered { .. } => {}
        }
    }
}

fn decrement_open_order(
    open_orders: &mut HashMap<Uuid, OpenOrder>,
    order_id: Uuid,
    quantity: Decimal,
) {
    let Some(meta) = open_orders.get_mut(&order_id) else {
        return;
    };
    if meta.remaining <= quantity {
        open_orders.remove(&order_id);
    } else {
        meta.remaining -= quantity;
    }
}

fn depth_levels(book: &OrderBook, depth: usize) -> (Vec<PriceLevel>, Vec<PriceLevel>) {
    let depth = book.depth_levels(depth);
    (depth.bids, depth.asks)
}
