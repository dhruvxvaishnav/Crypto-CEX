use std::io;
use std::net::SocketAddr;

use cex_proto::FrameError;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinError;

/// Errors emitted by the engine TCP server.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Engine must bind only to loopback per PRD §12.1.
    #[error("engine server refuses non-loopback bind address {0}")]
    NonLoopbackBind(SocketAddr),
    /// Underlying IO failed.
    #[error("server io error")]
    Io(#[from] io::Error),
    /// Wire frame encoding or decoding failed.
    #[error("server frame error")]
    Frame(#[from] FrameError),
    /// A connection writer task stopped unexpectedly.
    #[error("connection writer stopped")]
    WriterStopped,
    /// A spawned task failed.
    #[error("server task join error")]
    Join(#[from] JoinError),
    /// Sending a response to the connection writer failed.
    #[error("response channel closed")]
    ResponseChannelClosed,
}

impl<T> From<mpsc::error::SendError<T>> for ServerError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ResponseChannelClosed
    }
}
