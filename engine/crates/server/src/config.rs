use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Runtime configuration for the engine TCP server.
#[derive(Debug, Clone)]
pub struct EngineServerConfig {
    /// Loopback address the TCP listener binds to.
    pub bind_addr: SocketAddr,
    /// Command WAL used to rebuild in-memory books on restart.
    pub command_wal_path: PathBuf,
    /// Event log used to preserve the global event sequence before broadcast.
    pub event_log_path: PathBuf,
    /// Maximum book levels included in generated book-delta events.
    pub snapshot_depth: usize,
}

impl Default for EngineServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7878),
            command_wal_path: PathBuf::from("engine.wal"),
            event_log_path: PathBuf::from("engine-events.wal"),
            snapshot_depth: 50,
        }
    }
}
