use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use thiserror::Error;

const DEFAULT_BRIDGE_RECONNECT_MS: u64 = 500;
const DEFAULT_ENGINE_PORT: u16 = 7_878;
const DEFAULT_POLL_INTERVAL_MS: u64 = 250;

/// Runtime configuration for the settlement worker.
#[derive(Debug, Clone)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,
    /// Engine TCP address used by the event bridge.
    pub engine_addr: SocketAddr,
    /// Delay before reconnecting the engine event bridge.
    pub bridge_reconnect_interval: Duration,
    /// Idle polling interval when no notification arrives.
    pub poll_interval: Duration,
    /// OTLP gRPC endpoint for distributed tracing.
    pub otlp_endpoint: Option<String>,
}

/// Config loading error.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Required environment variable is missing.
    #[error("missing required env var {0}")]
    Missing(&'static str),
    /// Environment variable could not be parsed.
    #[error("invalid env var {name}: {reason}")]
    Invalid {
        /// Environment variable name.
        name: &'static str,
        /// Human-readable parse failure.
        reason: String,
    },
}

impl Config {
    /// Loads settlement configuration from process environment.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if required values are missing or malformed.
    pub fn from_env() -> Result<Self, ConfigError> {
        let poll_ms = optional("SETTLEMENT_POLL_INTERVAL_MS").map_or(
            Ok(DEFAULT_POLL_INTERVAL_MS),
            |value| {
                value.parse::<u64>().map_err(|error| ConfigError::Invalid {
                    name: "SETTLEMENT_POLL_INTERVAL_MS",
                    reason: error.to_string(),
                })
            },
        )?;

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            engine_addr: SocketAddr::new(
                parse_ip(
                    "ENGINE_HOST",
                    &optional("ENGINE_HOST").unwrap_or_else(|| "127.0.0.1".to_owned()),
                )?,
                parse_u16("ENGINE_PORT", DEFAULT_ENGINE_PORT)?,
            ),
            bridge_reconnect_interval: Duration::from_millis(parse_ms(
                "SETTLEMENT_BRIDGE_RECONNECT_MS",
                DEFAULT_BRIDGE_RECONNECT_MS,
            )?),
            poll_interval: Duration::from_millis(poll_ms),
            otlp_endpoint: optional("OTLP_ENDPOINT"),
        })
    }
}

fn optional(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    optional(name).ok_or(ConfigError::Missing(name))
}

fn parse_ip(name: &'static str, value: &str) -> Result<IpAddr, ConfigError> {
    value.parse::<IpAddr>().or_else(|_| {
        if value == "localhost" {
            Ok(IpAddr::V4(Ipv4Addr::LOCALHOST))
        } else {
            Err(ConfigError::Invalid {
                name,
                reason: "must be an IP address or localhost".to_owned(),
            })
        }
    })
}

fn parse_ms(name: &'static str, default: u64) -> Result<u64, ConfigError> {
    optional(name).map_or(Ok(default), |value| {
        value.parse::<u64>().map_err(|error| ConfigError::Invalid {
            name,
            reason: error.to_string(),
        })
    })
}

fn parse_u16(name: &'static str, default: u16) -> Result<u16, ConfigError> {
    optional(name).map_or(Ok(default), |value| {
        value.parse::<u16>().map_err(|error| ConfigError::Invalid {
            name,
            reason: error.to_string(),
        })
    })
}
