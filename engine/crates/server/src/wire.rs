use cex_core::book::EngineEvent as CoreEngineEvent;
use cex_core::types::{
    CancelReason as CoreCancelReason, Order, OrderType as CoreOrderType, Side as CoreSide,
    StpMode as CoreStpMode,
};
use cex_proto::{
    CancelReason, EngineOrder, EngineOrderType, EngineResponse, OrderSide, OrderStatus,
    RejectResponse, StpMode,
};
use rust_decimal::Decimal;
use uuid::Uuid;

const FOK_NOT_FILLED: &str = "FOK_NOT_FILLED";
const MARKET_HALTED: &str = "MARKET_HALTED";
const MARKET_UNKNOWN: &str = "MARKET_UNKNOWN";
const MIN_NOTIONAL: &str = "MIN_NOTIONAL";
const ORDER_NOT_FOUND: &str = "ORDER_NOT_FOUND";
const POST_ONLY_REJECTED: &str = "POST_ONLY_REJECTED";
const STOP_PRICE_INVALID: &str = "STOP_PRICE_INVALID";
const TICK_SIZE: &str = "TICK_SIZE";

pub(crate) fn market_halted(request_id: Uuid) -> super::state::ResponseBundle {
    reject(
        request_id,
        MARKET_HALTED,
        "market is halted and cannot accept new orders",
    )
}

pub(crate) fn order_not_found(
    request_id: Uuid,
    message: &'static str,
) -> super::state::ResponseBundle {
    reject(request_id, ORDER_NOT_FOUND, message)
}

pub(crate) fn validate_wire_order(request_id: Uuid, order: &EngineOrder) -> Option<RejectResponse> {
    if order.symbol.trim().is_empty() {
        return Some(reject_response(
            request_id,
            MARKET_UNKNOWN,
            "market symbol is required",
        ));
    }
    if order.quantity <= Decimal::ZERO {
        return Some(reject_response(
            request_id,
            MIN_NOTIONAL,
            "quantity must be positive",
        ));
    }
    if requires_price(order.order_type)
        && order.price.filter(|price| *price > Decimal::ZERO).is_none()
    {
        return Some(reject_response(
            request_id,
            TICK_SIZE,
            "positive price is required",
        ));
    }
    if requires_stop_price(order.order_type)
        && order
            .stop_price
            .filter(|price| *price > Decimal::ZERO)
            .is_none()
    {
        return Some(reject_response(
            request_id,
            STOP_PRICE_INVALID,
            "positive stop price is required",
        ));
    }
    if order.order_type == EngineOrderType::Iceberg
        && order
            .display_qty
            .filter(|quantity| *quantity > Decimal::ZERO && *quantity < order.quantity)
            .is_none()
    {
        return Some(reject_response(
            request_id,
            MIN_NOTIONAL,
            "iceberg display quantity must be positive and smaller than quantity",
        ));
    }
    None
}

pub(crate) fn immediate_pre_reject(
    request_id: Uuid,
    order_id: Uuid,
    events: &[CoreEngineEvent],
) -> Option<RejectResponse> {
    events.iter().find_map(|event| match event {
        CoreEngineEvent::Cancelled {
            order_id: id,
            reason,
        } if *id == order_id => match reason {
            CoreCancelReason::FokUnfilled => Some(reject_response(
                request_id,
                FOK_NOT_FILLED,
                "fill-or-kill order could not be fully filled",
            )),
            CoreCancelReason::PostOnlyCrossed => Some(reject_response(
                request_id,
                POST_ONLY_REJECTED,
                "post-only order would cross the book",
            )),
            _ => None,
        },
        _ => None,
    })
}

pub(crate) fn order_status_for_place(
    order_id: Uuid,
    quantity: Decimal,
    events: &[CoreEngineEvent],
) -> OrderStatus {
    let filled = events
        .iter()
        .filter_map(|event| match event {
            CoreEngineEvent::Fill(fill) if fill.taker_order_id == order_id => Some(fill.quantity),
            _ => None,
        })
        .sum::<Decimal>();
    let has_rested = events.iter().any(|event| {
        matches!(
            event,
            CoreEngineEvent::Rested {
                order_id: rested_id,
                ..
            } if *rested_id == order_id
        )
    });
    if has_rested {
        return if filled > Decimal::ZERO {
            OrderStatus::Partial
        } else {
            OrderStatus::New
        };
    }
    if filled >= quantity {
        return OrderStatus::Filled;
    }
    if events.iter().any(|event| {
        matches!(
            event,
            CoreEngineEvent::Cancelled {
                order_id: canceled_id,
                ..
            } if *canceled_id == order_id
        )
    }) {
        return OrderStatus::Canceled;
    }
    OrderStatus::Filled
}

pub(crate) fn cancelled_order_ids(events: &[CoreEngineEvent]) -> Vec<Uuid> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEngineEvent::Cancelled { order_id, .. } => Some(*order_id),
            _ => None,
        })
        .collect()
}

pub(crate) fn reject(
    request_id: Uuid,
    code: &'static str,
    message: &'static str,
) -> super::state::ResponseBundle {
    super::state::ResponseBundle {
        response: EngineResponse::Reject(reject_response(request_id, code, message)),
        events: Vec::new(),
    }
}

pub(crate) fn reject_response(
    request_id: Uuid,
    code: &'static str,
    message: &'static str,
) -> RejectResponse {
    RejectResponse {
        request_id,
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

pub(crate) fn last_seq_or_current(events: &[cex_proto::SequencedEngineEvent], current: u64) -> u64 {
    events.last().map_or(current, |event| event.seq)
}

pub(crate) fn to_core_order(order: &EngineOrder) -> Order {
    Order {
        id: order.id,
        user_id: order.user_id,
        symbol: order.symbol.clone().into(),
        side: to_core_side(order.side),
        order_type: to_core_order_type(order.order_type),
        price: order.price,
        quantity: order.quantity,
        remaining: order.quantity,
        stp_mode: to_core_stp_mode(order.stp_mode),
        stop_price: order.stop_price,
        oco_linked_id: order.oco_linked_id,
        display_qty: order.display_qty,
    }
}

pub(crate) fn to_wire_side(side: CoreSide) -> OrderSide {
    match side {
        CoreSide::Buy => OrderSide::Buy,
        CoreSide::Sell => OrderSide::Sell,
    }
}

pub(crate) fn to_wire_cancel_reason(reason: CoreCancelReason) -> CancelReason {
    match reason {
        CoreCancelReason::NoLiquidity => CancelReason::NoLiquidity,
        CoreCancelReason::FokUnfilled => CancelReason::FokUnfilled,
        CoreCancelReason::PostOnlyCrossed => CancelReason::PostOnlyCrossed,
        CoreCancelReason::SelfTradePrevented => CancelReason::SelfTradePrevented,
        CoreCancelReason::Oco => CancelReason::Oco,
        CoreCancelReason::AdminCancel => CancelReason::AdminCancel,
    }
}

fn requires_price(order_type: EngineOrderType) -> bool {
    matches!(
        order_type,
        EngineOrderType::Limit
            | EngineOrderType::Ioc
            | EngineOrderType::Fok
            | EngineOrderType::PostOnly
            | EngineOrderType::StopLimit
            | EngineOrderType::Oco
            | EngineOrderType::Iceberg
    )
}

fn requires_stop_price(order_type: EngineOrderType) -> bool {
    matches!(
        order_type,
        EngineOrderType::StopLimit | EngineOrderType::StopMarket
    )
}

fn to_core_side(side: OrderSide) -> CoreSide {
    match side {
        OrderSide::Buy => CoreSide::Buy,
        OrderSide::Sell => CoreSide::Sell,
    }
}

fn to_core_order_type(order_type: EngineOrderType) -> CoreOrderType {
    match order_type {
        EngineOrderType::Limit => CoreOrderType::Limit,
        EngineOrderType::Market => CoreOrderType::Market,
        EngineOrderType::Ioc => CoreOrderType::Ioc,
        EngineOrderType::Fok => CoreOrderType::Fok,
        EngineOrderType::PostOnly => CoreOrderType::PostOnly,
        EngineOrderType::StopLimit => CoreOrderType::StopLimit,
        EngineOrderType::StopMarket => CoreOrderType::StopMarket,
        EngineOrderType::Oco => CoreOrderType::Oco,
        EngineOrderType::Iceberg => CoreOrderType::Iceberg,
    }
}

fn to_core_stp_mode(mode: StpMode) -> CoreStpMode {
    match mode {
        StpMode::Decrement => CoreStpMode::Decrement,
        StpMode::CancelMaker => CoreStpMode::CancelMaker,
        StpMode::CancelTaker => CoreStpMode::CancelTaker,
    }
}
