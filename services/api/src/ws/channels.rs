/// Channel name for book diff updates: `book.<symbol>.diff`
#[must_use]
pub fn book_diff(symbol: &str) -> String {
    format!("book.{symbol}.diff")
}

/// Channel name for public trade events: `trade.<symbol>`
#[must_use]
pub fn trade(symbol: &str) -> String {
    format!("trade.{symbol}")
}

/// Channel name for 24h ticker: `ticker.<symbol>`
#[must_use]
pub fn ticker(symbol: &str) -> String {
    format!("ticker.{symbol}")
}

/// Channel name for the all-markets ticker batch.
pub const TICKER_ALL: &str = "ticker.all";

/// Channel name for user order updates (private).
pub const USER_ORDERS: &str = "user.orders";

/// Channel name for user fill updates (private).
pub const USER_FILLS: &str = "user.fills";

/// Channel name for user balance updates (private).
pub const USER_BALANCES: &str = "user.balances";
