use std::time::Duration;

use futures_util::StreamExt;
use rust_decimal::Decimal;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;

pub const BINANCE_COMBINED_STREAM_BASE: &str = "wss://stream.binance.com:9443/stream?streams=";
const DEFAULT_KLINE_INTERVAL: &str = "1m";
const RECONNECT_BASE_MS: u64 = 250;
const RECONNECT_MAX_MS: u64 = 5_000;
const DEFAULT_TOP_USDT_SYMBOLS: [&str; 50] = [
    "BTCUSDT",
    "ETHUSDT",
    "BNBUSDT",
    "SOLUSDT",
    "XRPUSDT",
    "DOGEUSDT",
    "ADAUSDT",
    "TRXUSDT",
    "AVAXUSDT",
    "LINKUSDT",
    "TONUSDT",
    "SHIBUSDT",
    "DOTUSDT",
    "BCHUSDT",
    "NEARUSDT",
    "MATICUSDT",
    "LTCUSDT",
    "UNIUSDT",
    "ICPUSDT",
    "APTUSDT",
    "ETCUSDT",
    "FILUSDT",
    "ATOMUSDT",
    "ARBUSDT",
    "OPUSDT",
    "INJUSDT",
    "SUIUSDT",
    "STXUSDT",
    "IMXUSDT",
    "HBARUSDT",
    "VETUSDT",
    "MKRUSDT",
    "RNDRUSDT",
    "AAVEUSDT",
    "GRTUSDT",
    "ARUSDT",
    "RUNEUSDT",
    "LDOUSDT",
    "THETAUSDT",
    "ALGOUSDT",
    "QNTUSDT",
    "FTMUSDT",
    "FLOWUSDT",
    "JASMYUSDT",
    "SEIUSDT",
    "WIFUSDT",
    "FETUSDT",
    "TIAUSDT",
    "PEPEUSDT",
    "BONKUSDT",
];

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

/// Closed or in-progress Binance kline update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineUpdate {
    /// Market symbol.
    pub symbol: String,
    /// Binance interval, for example `1m`.
    pub interval: String,
    /// Open time in Unix milliseconds.
    pub open_time_ms: i64,
    /// Close time in Unix milliseconds.
    pub close_time_ms: i64,
    /// Open price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Close price.
    pub close: Decimal,
    /// Base-asset volume.
    pub volume: Decimal,
    /// Whether Binance has closed this candle.
    pub is_closed: bool,
}

/// Market-data event emitted by Binance ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinanceEvent {
    /// Best bid/ask update.
    BookTicker(BookTicker),
    /// Kline update.
    Kline(KlineUpdate),
}

/// Default top-50 USDT symbols used for Day 5 Binance ingestion.
#[must_use]
pub fn default_top_usdt_symbols() -> Vec<String> {
    DEFAULT_TOP_USDT_SYMBOLS
        .iter()
        .map(|symbol| (*symbol).to_owned())
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CombinedBookTicker {
    data: RawBookTicker,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CombinedKline {
    data: RawKlineEnvelope,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawKlineEnvelope {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "k")]
    kline: RawKline,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawKline {
    #[serde(rename = "t")]
    open_time_ms: i64,
    #[serde(rename = "T")]
    close_time_ms: i64,
    #[serde(rename = "i")]
    interval: String,
    #[serde(rename = "o")]
    open: Decimal,
    #[serde(rename = "c")]
    close: Decimal,
    #[serde(rename = "h")]
    high: Decimal,
    #[serde(rename = "l")]
    low: Decimal,
    #[serde(rename = "v")]
    volume: Decimal,
    #[serde(rename = "x")]
    is_closed: bool,
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

#[derive(Debug, Error)]
pub enum BinanceParseError {
    /// JSON did not match Binance book ticker shape.
    #[error("invalid binance book ticker payload: {0}")]
    Invalid(serde_json::Error),
}

#[must_use]
pub fn combined_book_ticker_url(symbols: &[String]) -> String {
    combined_stream_url(symbols, &[StreamKind::BookTicker])
}

#[must_use]
pub fn combined_market_data_url(symbols: &[String]) -> String {
    combined_stream_url(symbols, &[StreamKind::BookTicker, StreamKind::Kline1m])
}

pub async fn run_reconnecting_stream(symbols: Vec<String>, tx: mpsc::Sender<BinanceEvent>) {
    let url = combined_market_data_url(&symbols);
    let mut backoff = Duration::from_millis(RECONNECT_BASE_MS);
    loop {
        match run_once(&url, &tx).await {
            Ok(()) => {
                tracing::warn!("binance.stream.closed");
            }
            Err(error) => {
                tracing::warn!(error = %error, "binance.stream.failed");
            }
        }
        if tx.is_closed() {
            break;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_millis(RECONNECT_MAX_MS));
    }
}

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    BookTicker,
    Kline1m,
}

async fn run_once(url: &str, tx: &mpsc::Sender<BinanceEvent>) -> Result<(), BinanceStreamError> {
    let (mut stream, _) = connect_async(url)
        .await
        .map_err(BinanceStreamError::Connect)?;
    while let Some(message) = stream.next().await {
        let message = message.map_err(BinanceStreamError::WebSocket)?;
        if message.is_text() {
            let Some(text) = message.to_text().ok() else {
                continue;
            };
            match parse_event(text) {
                Ok(event) => {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
                Err(error) => {
                    tracing::debug!(error = %error, "binance.stream.message_skipped");
                }
            }
        }
    }
    Ok(())
}

fn combined_stream_url(symbols: &[String], kinds: &[StreamKind]) -> String {
    let streams = symbols
        .iter()
        .flat_map(|symbol| {
            kinds.iter().map(move |kind| match kind {
                StreamKind::BookTicker => format!("{}@bookTicker", symbol.to_ascii_lowercase()),
                StreamKind::Kline1m => {
                    format!(
                        "{}@kline_{DEFAULT_KLINE_INTERVAL}",
                        symbol.to_ascii_lowercase()
                    )
                }
            })
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("{BINANCE_COMBINED_STREAM_BASE}{streams}")
}

/// Parses any supported Binance combined-stream payload.
///
/// # Errors
///
/// Returns [`BinanceParseError`] if the payload is not a supported event.
pub fn parse_event(payload: &str) -> Result<BinanceEvent, BinanceParseError> {
    parse_book_ticker(payload)
        .map(BinanceEvent::BookTicker)
        .or_else(|_| parse_kline(payload).map(BinanceEvent::Kline))
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

/// Parses a Binance combined-stream kline payload.
///
/// # Errors
///
/// Returns [`BinanceParseError`] if the payload shape or decimals are invalid.
pub fn parse_kline(payload: &str) -> Result<KlineUpdate, BinanceParseError> {
    let raw = serde_json::from_str::<CombinedKline>(payload)
        .map_err(BinanceParseError::Invalid)?
        .data;
    Ok(KlineUpdate {
        symbol: raw.symbol,
        interval: raw.kline.interval,
        open_time_ms: raw.kline.open_time_ms,
        close_time_ms: raw.kline.close_time_ms,
        open: raw.kline.open,
        high: raw.kline.high,
        low: raw.kline.low,
        close: raw.kline.close,
        volume: raw.kline.volume,
        is_closed: raw.kline.is_closed,
    })
}

#[derive(Debug, Error)]
pub enum BinanceStreamError {
    /// WebSocket connection failed.
    #[error("binance websocket connect failed: {0}")]
    Connect(tokio_tungstenite::tungstenite::Error),
    /// WebSocket frame failed.
    #[error("binance websocket frame failed: {0}")]
    WebSocket(tokio_tungstenite::tungstenite::Error),
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{
        combined_book_ticker_url, combined_market_data_url, default_top_usdt_symbols,
        parse_book_ticker, parse_kline,
    };

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
    fn builds_combined_market_data_url() {
        let symbols = vec!["BTCUSDT".to_owned()];

        let url = combined_market_data_url(&symbols);

        assert_eq!(
            url,
            "wss://stream.binance.com:9443/stream?streams=btcusdt@bookTicker/btcusdt@kline_1m"
        );
    }

    #[test]
    fn default_top_usdt_stream_has_fifty_symbols() {
        let symbols = default_top_usdt_symbols();

        assert_eq!(symbols.len(), 50);
        assert!(symbols.iter().all(|symbol| symbol.ends_with("USDT")));
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

    #[test]
    fn parses_combined_kline_payload() {
        let payload = r#"{"stream":"btcusdt@kline_1m","data":{"s":"BTCUSDT","k":{"t":1000,"T":59999,"i":"1m","o":"100.0","c":"101.0","h":"102.0","l":"99.0","v":"12.5","x":true}}}"#;

        let parsed = parse_kline(payload);
        assert!(parsed.is_ok());
        let Some(kline) = parsed.ok() else {
            return;
        };

        assert_eq!(kline.symbol, "BTCUSDT");
        assert_eq!(kline.interval, "1m");
        assert_eq!(kline.volume, Decimal::new(125, 1));
        assert!(kline.is_closed);
    }
}
