use cex_proto::{EngineEvent, SequencedEngineEvent};
use serde_json::Value;
use thiserror::Error;

/// Engine event row fetched from Postgres.
#[derive(Debug, Clone)]
pub struct StoredEngineEvent {
    /// Global engine sequence.
    pub seq: i64,
    /// Stable event type string.
    pub event_type: String,
    /// JSON event payload.
    pub payload: Value,
}

/// Event decoding failure.
#[derive(Debug, Error)]
pub enum EventDecodeError {
    /// JSON payload did not match any supported engine event shape.
    #[error("invalid engine event payload: {0}")]
    Invalid(serde_json::Error),
}

impl StoredEngineEvent {
    /// Decodes the stored payload into the canonical engine event enum.
    ///
    /// # Errors
    ///
    /// Returns [`EventDecodeError`] when the JSON shape is not supported.
    pub fn decode(&self) -> Result<EngineEvent, EventDecodeError> {
        serde_json::from_value::<EngineEvent>(self.payload.clone()).or_else(|first_error| {
            serde_json::from_value::<SequencedEngineEvent>(self.payload.clone())
                .map(|sequenced| sequenced.event)
                .map_err(|_| EventDecodeError::Invalid(first_error))
        })
    }
}
