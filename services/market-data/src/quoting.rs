use rust_decimal::Decimal;

use crate::binance::BookTicker;

const BPS_DENOMINATOR: i64 = 10_000;
const DEFAULT_LEVELS: usize = 10;

/// One generated market-maker quote level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteLevel {
    /// Quote price.
    pub price: Decimal,
    /// Quote quantity.
    pub quantity: Decimal,
}

/// Symmetric 10x10 quote set around an external mid price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteSet {
    /// Bid levels, best first.
    pub bids: Vec<QuoteLevel>,
    /// Ask levels, best first.
    pub asks: Vec<QuoteLevel>,
}

/// Deterministic quote generator for the demo market-maker.
#[derive(Debug, Clone)]
pub struct QuoteGenerator {
    levels: usize,
    spread_bps: Decimal,
    base_quantity: Decimal,
}

impl QuoteGenerator {
    /// Creates a quote generator using PRD Day 5 defaults.
    #[must_use]
    pub fn default_10x10() -> Self {
        Self {
            levels: DEFAULT_LEVELS,
            spread_bps: Decimal::new(5, 0),
            base_quantity: Decimal::new(1, 0),
        }
    }

    /// Builds bid and ask quote levels around a Binance book ticker.
    #[must_use]
    pub fn generate(&self, ticker: &BookTicker) -> QuoteSet {
        let mid = (ticker.bid_price + ticker.ask_price) / Decimal::new(2, 0);
        let mut bids = Vec::with_capacity(self.levels);
        let mut asks = Vec::with_capacity(self.levels);

        for level in 1..=self.levels {
            let level_decimal = Decimal::new(i64::try_from(level).unwrap_or(0), 0);
            let offset = self.spread_bps * level_decimal / Decimal::new(BPS_DENOMINATOR, 0);
            let quantity = self.base_quantity * level_decimal;
            bids.push(QuoteLevel {
                price: mid * (Decimal::ONE - offset),
                quantity,
            });
            asks.push(QuoteLevel {
                price: mid * (Decimal::ONE + offset),
                quantity,
            });
        }

        QuoteSet { bids, asks }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::binance::BookTicker;

    use super::QuoteGenerator;

    #[test]
    fn generates_ten_bid_and_ask_levels_around_mid() {
        let ticker = BookTicker {
            symbol: "BTCUSDT".to_owned(),
            bid_price: Decimal::new(100, 0),
            bid_quantity: Decimal::new(1, 0),
            ask_price: Decimal::new(102, 0),
            ask_quantity: Decimal::new(1, 0),
        };
        let generator = QuoteGenerator::default_10x10();

        let quotes = generator.generate(&ticker);

        assert_eq!(quotes.bids.len(), 10);
        assert_eq!(quotes.asks.len(), 10);
        let Some(best_bid) = quotes.bids.first() else {
            return;
        };
        let Some(worst_bid) = quotes.bids.last() else {
            return;
        };
        let Some(best_ask) = quotes.asks.first() else {
            return;
        };
        let Some(worst_ask) = quotes.asks.last() else {
            return;
        };
        assert!(best_bid.price < Decimal::new(101, 0));
        assert!(best_ask.price > Decimal::new(101, 0));
        assert!(best_bid.price > worst_bid.price);
        assert!(best_ask.price < worst_ask.price);
    }
}
