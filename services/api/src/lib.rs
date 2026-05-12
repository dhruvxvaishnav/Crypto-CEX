//! Aether API gateway.

pub mod auth;
pub mod clock;
pub mod config;
pub mod engine_client;
pub mod errors;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod openapi;
pub mod rate_limit;
pub mod readiness;
pub mod repositories;
pub mod router;
pub mod schemas;
pub mod security_headers;
pub mod state;
pub mod ws;

pub use router::build_router;
pub use state::AppState;
