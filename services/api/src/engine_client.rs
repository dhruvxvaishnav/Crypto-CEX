use std::net::SocketAddr;
use std::time::Duration;

use cex_proto::{read_json_frame, write_json_frame, EngineRequest, EngineResponse, PingRequest};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::time::timeout;
use uuid::Uuid;

/// TCP client for the matching engine.
#[derive(Debug, Clone)]
pub struct EngineClient {
    addr: SocketAddr,
    timeout: Duration,
}

/// Engine client error.
#[derive(Debug, Error)]
pub enum EngineClientError {
    /// Connection or frame IO failed.
    #[error("engine io failed")]
    Io,
    /// Engine timed out.
    #[error("engine timed out")]
    Timeout,
    /// Engine returned an unexpected response.
    #[error("unexpected engine response")]
    Unexpected,
}

impl EngineClient {
    /// Creates a new engine TCP client.
    #[must_use]
    pub const fn new(addr: SocketAddr, timeout: Duration) -> Self {
        Self { addr, timeout }
    }

    /// Pings the engine.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] when the engine cannot be reached or responds incorrectly.
    pub async fn ping(&self, request_id: Uuid) -> Result<(), EngineClientError> {
        let addr = self.addr;
        let timeout_duration = self.timeout;
        timeout(timeout_duration, async move {
            let mut stream = TcpStream::connect(addr)
                .await
                .map_err(|_| EngineClientError::Io)?;
            write_json_frame(
                &mut stream,
                &EngineRequest::Ping(PingRequest { request_id }),
            )
            .await
            .map_err(|_| EngineClientError::Io)?;
            match read_json_frame::<_, EngineResponse>(&mut stream)
                .await
                .map_err(|_| EngineClientError::Io)?
            {
                Some(EngineResponse::Pong(pong)) if pong.request_id == request_id => Ok(()),
                _ => Err(EngineClientError::Unexpected),
            }
        })
        .await
        .map_err(|_| EngineClientError::Timeout)?
    }
}
