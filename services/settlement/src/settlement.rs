use cex_proto::{
    EngineEvent, EngineOrder, EngineOrderType, OrderAcceptedEvent, OrderCanceledEvent, OrderSide,
    TradeFill,
};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::events::StoredEngineEvent;

const BPS_DENOMINATOR: i64 = 10_000;
const SETTLEMENT_WORKER: &str = "settlement";

/// Settlement processing error.
#[derive(Debug, Error)]
pub enum SettlementError {
    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Stored engine payload could not be decoded.
    #[error("event decode error: {0}")]
    Decode(#[from] crate::events::EventDecodeError),
    /// Engine sequence exceeded the database integer range.
    #[error("engine sequence is out of range")]
    SequenceOutOfRange,
}

/// Postgres-backed settlement worker.
#[derive(Debug, Clone)]
pub struct PostgresSettlement {
    pool: PgPool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct OrderRow {
    id: Uuid,
    user_id: Uuid,
    market_id: Uuid,
    side: String,
    quantity: Option<Decimal>,
    price: Option<Decimal>,
    filled_quantity: Decimal,
    avg_fill_price: Option<Decimal>,
    status: String,
    base_asset_id: Uuid,
    quote_asset_id: Uuid,
    maker_fee_bps: Decimal,
    taker_fee_bps: Decimal,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct MarketRow {
    id: Uuid,
    base_asset_id: Uuid,
    quote_asset_id: Uuid,
}

impl PostgresSettlement {
    /// Creates a settlement processor backed by `pool`.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Processes all currently unprocessed engine events.
    ///
    /// # Errors
    ///
    /// Returns [`SettlementError`] when event decoding or database writes fail.
    pub async fn process_available(&self) -> Result<u64, SettlementError> {
        let mut processed = 0_u64;
        while self.process_next().await? {
            processed = processed.saturating_add(1);
        }
        Ok(processed)
    }

    async fn process_next(&self) -> Result<bool, SettlementError> {
        let mut tx = self.pool.begin().await?;
        let Some(stored) = fetch_next_event(&mut tx).await? else {
            tx.commit().await?;
            return Ok(false);
        };
        let event = stored.decode()?;
        match event {
            EngineEvent::OrderAccepted(event) => {
                settle_order_accepted(&mut tx, stored.seq, event).await?;
            }
            EngineEvent::Fill(fill) => {
                settle_fill(&mut tx, stored.seq, fill).await?;
            }
            EngineEvent::OrderCanceled(event) => {
                settle_cancel(&mut tx, stored.seq, event).await?;
            }
            EngineEvent::OrderRested(_)
            | EngineEvent::BookDelta(_)
            | EngineEvent::MarketStatus(_) => {}
        }
        mark_processed(&mut tx, stored.seq).await?;
        tx.commit().await?;
        Ok(true)
    }
}

async fn fetch_next_event(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<StoredEngineEvent>, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, String, serde_json::Value)>(
        r"
        SELECT seq, event_type, payload
        FROM engine_events
        WHERE processed_at IS NULL
        ORDER BY seq
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        ",
    )
    .fetch_optional(tx.as_mut())
    .await?;

    Ok(row.map(|(seq, event_type, payload)| StoredEngineEvent {
        seq,
        event_type,
        payload,
    }))
}

async fn settle_order_accepted(
    tx: &mut Transaction<'_, Postgres>,
    seq: i64,
    event: OrderAcceptedEvent,
) -> Result<(), sqlx::Error> {
    let market = fetch_market(tx, &event.order.symbol).await?;
    insert_order(tx, &event.order, market.id).await?;
    let (asset_id, obligation) = lock_obligation(&event.order, &market);
    if obligation > Decimal::ZERO {
        apply_ledger(
            tx,
            LedgerMutation {
                user_id: event.order.user_id,
                asset_id,
                kind: "lock",
                amount: Decimal::ZERO,
                available_delta: -obligation,
                locked_delta: obligation,
                reference_type: "order",
                reference_id: Some(event.order.id),
                engine_seq: Some(seq),
            },
        )
        .await?;
    }
    Ok(())
}

async fn settle_fill(
    tx: &mut Transaction<'_, Postgres>,
    seq: i64,
    fill: TradeFill,
) -> Result<(), sqlx::Error> {
    let existing = sqlx::query_scalar::<_, i64>(
        r"
        SELECT COUNT(*)
        FROM trades
        WHERE engine_seq = $1
        ",
    )
    .bind(seq)
    .fetch_one(tx.as_mut())
    .await?;
    if existing > 0 {
        return Ok(());
    }

    let taker = fetch_order(tx, fill.taker_order_id).await?;
    let maker = fetch_order(tx, fill.maker_order_id).await?;
    insert_trade(tx, seq, &fill, taker.market_id).await?;
    apply_fill_ledger(tx, seq, &fill, &taker, &maker).await?;
    update_order_fill(tx, &taker, fill.price, fill.quantity).await?;
    update_order_fill(tx, &maker, fill.price, fill.quantity).await?;
    Ok(())
}

async fn insert_trade(
    tx: &mut Transaction<'_, Postgres>,
    seq: i64,
    fill: &TradeFill,
    market_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO trades
          (id, market_id, taker_order_id, maker_order_id, price, quantity, taker_side, engine_seq)
        VALUES ($1, $2, $3, $4, $5, $6, $7::order_side, $8)
        ON CONFLICT (engine_seq) DO NOTHING
        ",
    )
    .bind(fill.trade_id)
    .bind(market_id)
    .bind(fill.taker_order_id)
    .bind(fill.maker_order_id)
    .bind(fill.price)
    .bind(fill.quantity)
    .bind(order_side_db(fill.taker_side))
    .bind(seq)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn apply_fill_ledger(
    tx: &mut Transaction<'_, Postgres>,
    seq: i64,
    fill: &TradeFill,
    taker: &OrderRow,
    maker: &OrderRow,
) -> Result<(), sqlx::Error> {
    let notional = fill.price * fill.quantity;
    let taker_fee = fee_amount(fill.quantity, taker.taker_fee_bps);
    let maker_fee = fee_amount(notional, maker.maker_fee_bps);
    let (buyer, seller) = match fill.taker_side {
        OrderSide::Buy => (taker, maker),
        OrderSide::Sell => (maker, taker),
    };

    apply_ledger(
        tx,
        LedgerMutation {
            user_id: buyer.user_id,
            asset_id: buyer.base_asset_id,
            kind: "trade",
            amount: fill.quantity,
            available_delta: fill.quantity,
            locked_delta: Decimal::ZERO,
            reference_type: "trade",
            reference_id: Some(fill.trade_id),
            engine_seq: Some(seq),
        },
    )
    .await?;
    apply_ledger(
        tx,
        LedgerMutation {
            user_id: buyer.user_id,
            asset_id: buyer.base_asset_id,
            kind: "fee",
            amount: -taker_fee,
            available_delta: -taker_fee,
            locked_delta: Decimal::ZERO,
            reference_type: "trade",
            reference_id: Some(fill.trade_id),
            engine_seq: Some(seq),
        },
    )
    .await?;
    apply_ledger(
        tx,
        LedgerMutation {
            user_id: buyer.user_id,
            asset_id: buyer.quote_asset_id,
            kind: "trade",
            amount: -notional,
            available_delta: Decimal::ZERO,
            locked_delta: -notional,
            reference_type: "trade",
            reference_id: Some(fill.trade_id),
            engine_seq: Some(seq),
        },
    )
    .await?;
    apply_ledger(
        tx,
        LedgerMutation {
            user_id: seller.user_id,
            asset_id: seller.quote_asset_id,
            kind: "trade",
            amount: notional,
            available_delta: notional,
            locked_delta: Decimal::ZERO,
            reference_type: "trade",
            reference_id: Some(fill.trade_id),
            engine_seq: Some(seq),
        },
    )
    .await?;
    apply_ledger(
        tx,
        LedgerMutation {
            user_id: seller.user_id,
            asset_id: seller.quote_asset_id,
            kind: "fee",
            amount: -maker_fee,
            available_delta: -maker_fee,
            locked_delta: Decimal::ZERO,
            reference_type: "trade",
            reference_id: Some(fill.trade_id),
            engine_seq: Some(seq),
        },
    )
    .await?;
    apply_ledger(
        tx,
        LedgerMutation {
            user_id: seller.user_id,
            asset_id: seller.base_asset_id,
            kind: "trade",
            amount: -fill.quantity,
            available_delta: Decimal::ZERO,
            locked_delta: -fill.quantity,
            reference_type: "trade",
            reference_id: Some(fill.trade_id),
            engine_seq: Some(seq),
        },
    )
    .await?;
    Ok(())
}

async fn settle_cancel(
    tx: &mut Transaction<'_, Postgres>,
    seq: i64,
    event: OrderCanceledEvent,
) -> Result<(), sqlx::Error> {
    let order = fetch_order(tx, event.order_id).await?;
    if matches!(order.status.as_str(), "canceled" | "filled") {
        return Ok(());
    }

    let remaining = order
        .quantity
        .map_or(Decimal::ZERO, |quantity| quantity - order.filled_quantity);
    let obligation = match order.side.as_str() {
        "buy" => order.price.map_or(Decimal::ZERO, |price| price * remaining),
        _ => remaining,
    };
    if obligation > Decimal::ZERO {
        let asset_id = if order.side == "buy" {
            order.quote_asset_id
        } else {
            order.base_asset_id
        };
        apply_ledger(
            tx,
            LedgerMutation {
                user_id: order.user_id,
                asset_id,
                kind: "unlock",
                amount: Decimal::ZERO,
                available_delta: obligation,
                locked_delta: -obligation,
                reference_type: "order",
                reference_id: Some(event.order_id),
                engine_seq: Some(seq),
            },
        )
        .await?;
    }

    sqlx::query(
        r"
        UPDATE orders
        SET status = 'canceled', updated_at = now()
        WHERE id = $1
        ",
    )
    .bind(event.order_id)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn fetch_market(
    tx: &mut Transaction<'_, Postgres>,
    symbol: &str,
) -> Result<MarketRow, sqlx::Error> {
    sqlx::query_as::<_, MarketRow>(
        r"
        SELECT id, base_asset_id, quote_asset_id
        FROM markets
        WHERE symbol = $1
        ",
    )
    .bind(symbol)
    .fetch_one(tx.as_mut())
    .await
}

async fn insert_order(
    tx: &mut Transaction<'_, Postgres>,
    order: &EngineOrder,
    market_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO orders
          (id, user_id, market_id, side, type, status, price, quantity, stop_price, display_quantity)
        VALUES
          ($1, $2, $3, $4::order_side, $5::order_type, 'new', $6, $7, $8, $9)
        ON CONFLICT (id) DO NOTHING
        ",
    )
    .bind(order.id)
    .bind(order.user_id)
    .bind(market_id)
    .bind(order_side_db(order.side))
    .bind(order_type_db(order.order_type))
    .bind(order.price)
    .bind(order.quantity)
    .bind(order.stop_price)
    .bind(order.display_qty)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

async fn fetch_order(
    tx: &mut Transaction<'_, Postgres>,
    order_id: Uuid,
) -> Result<OrderRow, sqlx::Error> {
    sqlx::query_as::<_, OrderRow>(
        r"
        SELECT
          orders.id,
          orders.user_id,
          orders.market_id,
          orders.side::TEXT AS side,
          orders.quantity,
          orders.price,
          orders.filled_quantity,
          orders.avg_fill_price,
          orders.status::TEXT AS status,
          markets.base_asset_id,
          markets.quote_asset_id,
          markets.maker_fee_bps,
          markets.taker_fee_bps
        FROM orders
        JOIN markets ON markets.id = orders.market_id
        WHERE orders.id = $1
        FOR UPDATE
        ",
    )
    .bind(order_id)
    .fetch_one(tx.as_mut())
    .await
}

async fn update_order_fill(
    tx: &mut Transaction<'_, Postgres>,
    order: &OrderRow,
    price: Decimal,
    quantity: Decimal,
) -> Result<(), sqlx::Error> {
    let new_filled = order.filled_quantity + quantity;
    let old_notional = order.avg_fill_price.unwrap_or(Decimal::ZERO) * order.filled_quantity;
    let avg_fill_price = (old_notional + (price * quantity)) / new_filled;
    let status = order
        .quantity
        .filter(|total| new_filled >= *total)
        .map_or("partial", |_| "filled");

    sqlx::query(
        r"
        UPDATE orders
        SET filled_quantity = $2,
            avg_fill_price = $3,
            status = $4::order_status,
            updated_at = now()
        WHERE id = $1
        ",
    )
    .bind(order.id)
    .bind(new_filled)
    .bind(avg_fill_price)
    .bind(status)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

struct LedgerMutation<'a> {
    user_id: Uuid,
    asset_id: Uuid,
    kind: &'a str,
    amount: Decimal,
    available_delta: Decimal,
    locked_delta: Decimal,
    reference_type: &'a str,
    reference_id: Option<Uuid>,
    engine_seq: Option<i64>,
}

async fn apply_ledger(
    tx: &mut Transaction<'_, Postgres>,
    mutation: LedgerMutation<'_>,
) -> Result<(), sqlx::Error> {
    let balance = sqlx::query_as::<_, (Decimal, Decimal)>(
        r"
        UPDATE balances
        SET available = available + $3,
            locked = locked + $4,
            updated_at = now()
        WHERE user_id = $1 AND asset_id = $2
        RETURNING available, locked
        ",
    )
    .bind(mutation.user_id)
    .bind(mutation.asset_id)
    .bind(mutation.available_delta)
    .bind(mutation.locked_delta)
    .fetch_one(tx.as_mut())
    .await?;

    sqlx::query(
        r"
        INSERT INTO ledger_entries
          (user_id, asset_id, kind, amount, available_after, locked_after,
           reference_type, reference_id, engine_seq)
        VALUES ($1, $2, $3::ledger_entry_kind, $4, $5, $6, $7, $8, $9)
        ",
    )
    .bind(mutation.user_id)
    .bind(mutation.asset_id)
    .bind(mutation.kind)
    .bind(mutation.amount)
    .bind(balance.0)
    .bind(balance.1)
    .bind(mutation.reference_type)
    .bind(mutation.reference_id)
    .bind(mutation.engine_seq)
    .execute(tx.as_mut())
    .await?;

    Ok(())
}

async fn mark_processed(tx: &mut Transaction<'_, Postgres>, seq: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        UPDATE engine_events
        SET processed_at = now()
        WHERE seq = $1
        ",
    )
    .bind(seq)
    .execute(tx.as_mut())
    .await?;

    sqlx::query(
        r"
        INSERT INTO worker_state (worker_name, last_processed_seq, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (worker_name)
        DO UPDATE SET
          last_processed_seq = GREATEST(worker_state.last_processed_seq, EXCLUDED.last_processed_seq),
          updated_at = now()
        ",
    )
    .bind(SETTLEMENT_WORKER)
    .bind(seq)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

fn lock_obligation(order: &EngineOrder, market: &MarketRow) -> (Uuid, Decimal) {
    match order.side {
        OrderSide::Buy => (
            market.quote_asset_id,
            order
                .price
                .map_or(Decimal::ZERO, |price| price * order.quantity),
        ),
        OrderSide::Sell => (market.base_asset_id, order.quantity),
    }
}

fn fee_amount(gross: Decimal, fee_bps: Decimal) -> Decimal {
    gross * fee_bps / Decimal::new(BPS_DENOMINATOR, 0)
}

const fn order_side_db(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

const fn order_type_db(order_type: EngineOrderType) -> &'static str {
    match order_type {
        EngineOrderType::Limit | EngineOrderType::Iceberg => "limit",
        EngineOrderType::Market => "market",
        EngineOrderType::Ioc => "ioc",
        EngineOrderType::Fok => "fok",
        EngineOrderType::PostOnly => "post_only",
        EngineOrderType::StopLimit => "stop_limit",
        EngineOrderType::StopMarket => "stop_market",
        EngineOrderType::Oco => "oco",
    }
}

#[cfg(test)]
mod tests {
    use super::fee_amount;
    use rust_decimal::Decimal;

    #[derive(Debug, Clone, Copy, Default)]
    struct Account {
        available: Decimal,
        locked: Decimal,
        ledger_sum: Decimal,
    }

    impl Account {
        fn apply(&mut self, amount: Decimal, available_delta: Decimal, locked_delta: Decimal) {
            self.available += available_delta;
            self.locked += locked_delta;
            self.ledger_sum += amount;
        }

        fn invariant_holds(self) -> bool {
            self.available + self.locked == self.ledger_sum
                && self.available >= Decimal::ZERO
                && self.locked >= Decimal::ZERO
        }
    }

    #[test]
    fn fee_amount_uses_basis_points() {
        let fee = fee_amount(Decimal::new(2, 0), Decimal::new(10, 0));

        assert_eq!(fee, Decimal::new(2, 3));
    }

    #[test]
    fn simulated_fills_preserve_balance_ledger_invariant() {
        let mut buyer_base = Account::default();
        let mut buyer_quote = Account {
            available: Decimal::new(1_000_000, 0),
            ledger_sum: Decimal::new(1_000_000, 0),
            ..Account::default()
        };
        let mut seller_base = Account {
            available: Decimal::new(1_000, 0),
            ledger_sum: Decimal::new(1_000, 0),
            ..Account::default()
        };
        let mut seller_quote = Account::default();

        let price = Decimal::new(100, 0);
        let quantity = Decimal::new(1, 3);
        let fee_bps = Decimal::new(10, 0);
        for _ in 0..1_000 {
            let notional = price * quantity;
            let base_fee = fee_amount(quantity, fee_bps);
            let quote_fee = fee_amount(notional, fee_bps);

            buyer_quote.apply(Decimal::ZERO, -notional, notional);
            seller_base.apply(Decimal::ZERO, -quantity, quantity);

            buyer_base.apply(quantity, quantity, Decimal::ZERO);
            buyer_base.apply(-base_fee, -base_fee, Decimal::ZERO);
            buyer_quote.apply(-notional, Decimal::ZERO, -notional);
            seller_quote.apply(notional, notional, Decimal::ZERO);
            seller_quote.apply(-quote_fee, -quote_fee, Decimal::ZERO);
            seller_base.apply(-quantity, Decimal::ZERO, -quantity);
        }

        assert!(buyer_base.invariant_holds());
        assert!(buyer_quote.invariant_holds());
        assert!(seller_base.invariant_holds());
        assert!(seller_quote.invariant_holds());
    }
}
