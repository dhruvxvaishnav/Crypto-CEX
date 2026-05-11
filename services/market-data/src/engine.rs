use std::net::SocketAddr;
use std::time::Duration;

use cex_proto::{
    read_json_frame, write_json_frame, AckResult, CancelAllRequest, EngineOrder, EngineRequest,
    EngineResponse, PlaceRequest, StpMode,
};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::time::timeout;
use uuid::Uuid;

/// Engine client used by the market-maker worker.
#[derive(Debug, Clone)]
pub struct EngineClient {
    addr: SocketAddr,
    timeout: Duration,
}

/// Engine client error.
#[derive(Debug, Error)]
pub enum EngineClientError {
    /// Engine frame or TCP IO failed.
    #[error("engine io failed")]
    Io,
    /// Engine request timed out.
    #[error("engine request timed out")]
    Timeout,
    /// Engine rejected the request.
    #[error("engine rejected request: {0}")]
    Rejected(String),
    /// Engine returned a syntactically valid but unexpected response.
    #[error("unexpected engine response")]
    Unexpected,
}

impl EngineClient {
    /// Creates an engine TCP client.
    #[must_use]
    pub const fn new(addr: SocketAddr, timeout: Duration) -> Self {
        Self { addr, timeout }
    }

    /// Cancels all market-maker orders in one symbol.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] when the engine cannot complete the request.
    pub async fn cancel_all(&self, user_id: Uuid, symbol: &str) -> Result<(), EngineClientError> {
        let request_id = Uuid::new_v4();
        let request = EngineRequest::CancelAll(CancelAllRequest {
            request_id,
            user_id,
            symbol: Some(symbol.to_owned()),
        });
        match self.send(request).await? {
            EngineResponse::Ack(ack) if ack.request_id == request_id => match ack.result {
                AckResult::CancelAll(_) => Ok(()),
                _ => Err(EngineClientError::Unexpected),
            },
            EngineResponse::Reject(reject) => Err(EngineClientError::Rejected(reject.code)),
            _ => Err(EngineClientError::Unexpected),
        }
    }

    /// Places one market-maker quote order.
    ///
    /// # Errors
    ///
    /// Returns [`EngineClientError`] when the engine cannot complete the request.
    pub async fn place(&self, order: EngineOrder) -> Result<(), EngineClientError> {
        let request_id = Uuid::new_v4();
        let request = EngineRequest::Place(PlaceRequest { request_id, order });
        match self.send(request).await? {
            EngineResponse::Ack(ack) if ack.request_id == request_id => match ack.result {
                AckResult::Order(_) => Ok(()),
                _ => Err(EngineClientError::Unexpected),
            },
            EngineResponse::Reject(reject) => Err(EngineClientError::Rejected(reject.code)),
            _ => Err(EngineClientError::Unexpected),
        }
    }

    async fn send(&self, request: EngineRequest) -> Result<EngineResponse, EngineClientError> {
        let addr = self.addr;
        timeout(self.timeout, async move {
            let mut stream = TcpStream::connect(addr)
                .await
                .map_err(|_| EngineClientError::Io)?;
            write_json_frame(&mut stream, &request)
                .await
                .map_err(|_| EngineClientError::Io)?;
            read_non_event_response(&mut stream).await
        })
        .await
        .map_err(|_| EngineClientError::Timeout)?
    }
}

async fn read_non_event_response(
    stream: &mut TcpStream,
) -> Result<EngineResponse, EngineClientError> {
    loop {
        let Some(response) = read_json_frame::<_, EngineResponse>(stream)
            .await
            .map_err(|_| EngineClientError::Io)?
        else {
            return Err(EngineClientError::Unexpected);
        };
        if !matches!(response, EngineResponse::Event(_)) {
            return Ok(response);
        }
    }
}

/// ID source for market-maker order generation.
pub trait IdSource: Send + Sync {
    /// Returns the next order ID.
    fn next_uuid(&self) -> Uuid;
}

/// UUID v4 ID source used at the worker edge.
#[derive(Debug, Clone, Copy)]
pub struct UuidV4Source;

impl IdSource for UuidV4Source {
    fn next_uuid(&self) -> Uuid {
        Uuid::new_v4()
    }
}

/// Creates a post-only limit order for the demo market-maker.
#[must_use]
pub fn quote_order(
    id_source: &dyn IdSource,
    user_id: Uuid,
    symbol: &str,
    side: cex_proto::OrderSide,
    price: rust_decimal::Decimal,
    quantity: rust_decimal::Decimal,
) -> EngineOrder {
    EngineOrder {
        id: id_source.next_uuid(),
        user_id,
        symbol: symbol.to_owned(),
        side,
        order_type: cex_proto::EngineOrderType::PostOnly,
        price: Some(price),
        quantity,
        stp_mode: StpMode::Decrement,
        stop_price: None,
        oco_linked_id: None,
        display_qty: None,
    }
}
