use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use thiserror::Error;

const DEFAULT_ENGINE_TIMEOUT_MS: u64 = 200;
const DEFAULT_ENGINE_PORT: u16 = 7_878;
const DEFAULT_REFRESH_MS: u64 = 2_000;
const DEFAULT_STALE_AFTER_MS: u64 = 30_000;

/// Runtime configuration for market-data ingestion and quoting.
#[derive(Debug, Clone)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,
    /// Engine TCP address.
    pub engine_addr: SocketAddr,
    /// Engine request timeout.
    pub engine_timeout: Duration,
    /// Market-maker quote refresh interval.
    pub quote_refresh_interval: Duration,
    /// Maximum allowed upstream feed age before quoting stops.
    pub stale_after: Duration,
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
    /// Loads market-data configuration from process environment.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if required values are missing or malformed.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            engine_addr: SocketAddr::new(
                parse_ip(
                    "ENGINE_HOST",
                    &optional("ENGINE_HOST").unwrap_or_else(|| "127.0.0.1".to_owned()),
                )?,
                parse_u16("ENGINE_PORT", DEFAULT_ENGINE_PORT)?,
            ),
            engine_timeout: Duration::from_millis(parse_ms(
                "MARKET_DATA_ENGINE_TIMEOUT_MS",
                DEFAULT_ENGINE_TIMEOUT_MS,
            )?),
            quote_refresh_interval: Duration::from_millis(parse_ms(
                "MARKET_DATA_QUOTE_REFRESH_MS",
                DEFAULT_REFRESH_MS,
            )?),
            stale_after: Duration::from_millis(parse_ms(
                "MARKET_DATA_STALE_AFTER_MS",
                DEFAULT_STALE_AFTER_MS,
            )?),
        })
    }
}

fn parse_ms(name: &'static str, default: u64) -> Result<u64, ConfigError> {
    optional(name).map_or(Ok(default), |value| {
        value.parse::<u64>().map_err(|error| ConfigError::Invalid {
            name,
            reason: error.to_string(),
        })
    })
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

fn parse_u16(name: &'static str, default: u16) -> Result<u16, ConfigError> {
    optional(name).map_or(Ok(default), |value| {
        value.parse::<u16>().map_err(|error| ConfigError::Invalid {
            name,
            reason: error.to_string(),
        })
    })
}
