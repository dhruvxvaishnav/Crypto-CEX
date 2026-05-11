use rust_decimal::Decimal;
use serde::Deserialize;
use thiserror::Error;

/// Binance combined stream base URL for public market-data feeds.
pub const BINANCE_COMBINED_STREAM_BASE: &str = "wss://stream.binance.com:9443/stream?streams=";

/// Best bid/ask update for one market.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookTicker {
    /// Aether market symbol, for example `BTCUSDT`.
    pub symbol: String,
    /// Best bid price.
    pub bid_price: Decimal,
    /// Best bid quantity.
    pub bid_quantity: Decimal,
    /// Best ask price.
    pub ask_price: Decimal,
    /// Best ask quantity.
    pub ask_quantity: Decimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CombinedBookTicker {
    data: RawBookTicker,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBookTicker {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "b")]
    bid_price: Decimal,
    #[serde(rename = "B")]
    bid_quantity: Decimal,
    #[serde(rename = "a")]
    ask_price: Decimal,
    #[serde(rename = "A")]
    ask_quantity: Decimal,
}

/// Binance payload parse error.
#[derive(Debug, Error)]
pub enum BinanceParseError {
    /// JSON did not match Binance book ticker shape.
    #[error("invalid binance book ticker payload: {0}")]
    Invalid(serde_json::Error),
}

/// Builds a Binance combined book-ticker stream URL for active market symbols.
#[must_use]
pub fn combined_book_ticker_url(symbols: &[String]) -> String {
    let streams = symbols
        .iter()
        .map(|symbol| format!("{}@bookTicker", symbol.to_ascii_lowercase()))
        .collect::<Vec<_>>()
        .join("/");
    format!("{BINANCE_COMBINED_STREAM_BASE}{streams}")
}

/// Parses a Binance combined-stream book ticker payload.
///
/// # Errors
///
/// Returns [`BinanceParseError`] if the payload shape or decimals are invalid.
pub fn parse_book_ticker(payload: &str) -> Result<BookTicker, BinanceParseError> {
    let raw = serde_json::from_str::<CombinedBookTicker>(payload)
        .map_err(BinanceParseError::Invalid)?
        .data;
    Ok(BookTicker {
        symbol: raw.symbol,
        bid_price: raw.bid_price,
        bid_quantity: raw.bid_quantity,
        ask_price: raw.ask_price,
        ask_quantity: raw.ask_quantity,
    })
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{combined_book_ticker_url, parse_book_ticker};

    #[test]
    fn builds_combined_book_ticker_url() {
        let symbols = vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()];

        let url = combined_book_ticker_url(&symbols);

        assert_eq!(
            url,
            "wss://stream.binance.com:9443/stream?streams=btcusdt@bookTicker/ethusdt@bookTicker"
        );
    }

    #[test]
    fn parses_combined_book_ticker_payload() {
        let payload = r#"{"stream":"btcusdt@bookTicker","data":{"s":"BTCUSDT","b":"100.1","B":"2","a":"100.3","A":"3"}}"#;

        let parsed = parse_book_ticker(payload);
        assert!(parsed.is_ok());
        let Some(ticker) = parsed.ok() else {
            return;
        };

        assert_eq!(ticker.symbol, "BTCUSDT");
        assert_eq!(ticker.bid_price, Decimal::new(1001, 1));
        assert_eq!(ticker.ask_price, Decimal::new(1003, 1));
    }
}
