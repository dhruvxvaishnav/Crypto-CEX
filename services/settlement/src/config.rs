use std::env;
use std::time::Duration;

use thiserror::Error;

const DEFAULT_POLL_INTERVAL_MS: u64 = 250;

/// Runtime configuration for the settlement worker.
#[derive(Debug, Clone)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,
    /// Idle polling interval when no notification arrives.
    pub poll_interval: Duration,
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
            poll_interval: Duration::from_millis(poll_ms),
        })
    }
}

fn optional(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    optional(name).ok_or(ConfigError::Missing(name))
}
