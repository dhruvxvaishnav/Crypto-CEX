use std::net::SocketAddr;

use cex_proto::{read_json_frame, EngineEvent, EngineResponse, SequencedEngineEvent};
use sqlx::postgres::PgPool;
use thiserror::Error;
use tokio::net::TcpStream;

/// Engine event bridge that copies engine broadcasts into Postgres.
#[derive(Debug, Clone)]
pub struct EngineEventBridge {
    pool: PgPool,
    engine_addr: SocketAddr,
}

/// Engine bridge error.
#[derive(Debug, Error)]
pub enum EngineEventBridgeError {
    /// TCP connection or frame read failed.
    #[error("engine bridge io failed")]
    Io,
    /// Database write failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Engine sequence exceeded the database integer range.
    #[error("engine sequence is out of range")]
    SequenceOutOfRange,
    /// Engine closed the TCP stream.
    #[error("engine stream closed")]
    Closed,
}

impl EngineEventBridge {
    /// Creates an engine event bridge.
    #[must_use]
    pub const fn new(pool: PgPool, engine_addr: SocketAddr) -> Self {
        Self { pool, engine_addr }
    }

    /// Connects once to the engine and persists broadcast events until closed.
    ///
    /// # Errors
    ///
    /// Returns [`EngineEventBridgeError`] when the engine stream or database write fails.
    pub async fn run_once(&self) -> Result<(), EngineEventBridgeError> {
        let mut stream = TcpStream::connect(self.engine_addr)
            .await
            .map_err(|_| EngineEventBridgeError::Io)?;
        loop {
            let Some(response) = read_json_frame::<_, EngineResponse>(&mut stream)
                .await
                .map_err(|_| EngineEventBridgeError::Io)?
            else {
                return Err(EngineEventBridgeError::Closed);
            };
            if let EngineResponse::Event(event) = response {
                persist_event(&self.pool, event).await?;
            }
        }
    }
}

async fn persist_event(
    pool: &PgPool,
    event: SequencedEngineEvent,
) -> Result<(), EngineEventBridgeError> {
    let seq = i64::try_from(event.seq).map_err(|_| EngineEventBridgeError::SequenceOutOfRange)?;
    let event_type = event_type(&event.event);
    let payload = serde_json::to_value(event.event).map_err(|_| EngineEventBridgeError::Io)?;
    sqlx::query(
        r"
        INSERT INTO engine_events (seq, event_type, payload)
        VALUES ($1, $2, $3)
        ON CONFLICT (seq) DO NOTHING
        ",
    )
    .bind(seq)
    .bind(event_type)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
}

const fn event_type(event: &EngineEvent) -> &'static str {
    match event {
        EngineEvent::OrderAccepted(_) => "orderAccepted",
        EngineEvent::OrderRested(_) => "orderRested",
        EngineEvent::Fill(_) => "fill",
        EngineEvent::OrderCanceled(_) => "orderCanceled",
        EngineEvent::BookDelta(_) => "bookDelta",
        EngineEvent::MarketStatus(_) => "marketStatus",
    }
}

#[cfg(test)]
mod tests {
    use cex_proto::{BookDeltaEvent, EngineEvent};

    use super::event_type;

    #[test]
    fn maps_event_type_to_wire_discriminator() {
        let event = EngineEvent::BookDelta(BookDeltaEvent {
            symbol: "BTCUSDT".to_owned(),
            bids: Vec::new(),
            asks: Vec::new(),
        });

        assert_eq!(event_type(&event), "bookDelta");
    }
}
