//! TCP matching-engine server for Aether.

mod config;
mod error;
mod event_log;
mod server;
mod state;
mod time;
mod wire;

pub use config::EngineServerConfig;
pub use error::ServerError;
pub use server::EngineServer;
pub use time::{Clock, IdSource, SystemClock, UuidV4Source};
