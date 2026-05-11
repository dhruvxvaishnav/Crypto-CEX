use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use cex_proto::OrderSide;
use uuid::Uuid;

use crate::binance::BookTicker;
use crate::engine::{quote_order, EngineClient, EngineClientError, UuidV4Source};
use crate::quoting::QuoteGenerator;

/// Market-maker state and quote refresh loop.
#[derive(Debug)]
pub struct MarketMaker {
    engine: EngineClient,
    id_source: UuidV4Source,
    quote_generator: QuoteGenerator,
    quoted_symbols: HashSet<String>,
    user_id: Uuid,
    stale_after: Duration,
    latest: HashMap<String, TimedTicker>,
}

#[derive(Debug, Clone)]
struct TimedTicker {
    ticker: BookTicker,
    updated_at: Instant,
}

impl MarketMaker {
    /// Creates a market-maker.
    #[must_use]
    pub fn new(
        engine: EngineClient,
        user_id: Uuid,
        stale_after: Duration,
        quoted_symbols: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            engine,
            id_source: UuidV4Source,
            quote_generator: QuoteGenerator::default_10x10(),
            quoted_symbols: quoted_symbols.into_iter().collect(),
            user_id,
            stale_after,
            latest: HashMap::new(),
        }
    }

    /// Records the latest external book ticker for a symbol.
    pub fn record_ticker(&mut self, ticker: BookTicker) {
        if !self.quoted_symbols.contains(&ticker.symbol) {
            return;
        }
        self.latest.insert(
            ticker.symbol.clone(),
            TimedTicker {
                ticker,
                updated_at: Instant::now(),
            },
        );
    }

    /// Cancels and replaces quotes for every symbol with a fresh external mid.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] if the matching engine rejects or fails a request.
    pub async fn refresh_quotes(&self) -> Result<(), EngineClientError> {
        for (symbol, timed) in &self.latest {
            if timed.updated_at.elapsed() > self.stale_after {
                self.engine.cancel_all(self.user_id, symbol).await?;
                tracing::warn!(symbol, "market_maker.quote.stale_canceled");
                continue;
            }
            let quotes = self.quote_generator.generate(&timed.ticker);
            self.engine.cancel_all(self.user_id, symbol).await?;
            for bid in quotes.bids {
                let order = quote_order(
                    &self.id_source,
                    self.user_id,
                    symbol,
                    OrderSide::Buy,
                    bid.price,
                    bid.quantity,
                );
                self.engine.place(order).await?;
            }
            for ask in quotes.asks {
                let order = quote_order(
                    &self.id_source,
                    self.user_id,
                    symbol,
                    OrderSide::Sell,
                    ask.price,
                    ask.quantity,
                );
                self.engine.place(order).await?;
            }
            tracing::info!(symbol, "market_maker.quote.refreshed");
        }
        Ok(())
    }
}
