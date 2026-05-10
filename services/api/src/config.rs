use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use thiserror::Error;

/// Runtime configuration for the API service.
#[derive(Debug, Clone)]
pub struct Config {
    /// Deployment environment.
    pub app_env: AppEnv,
    /// HTTP bind address.
    pub bind_addr: SocketAddr,
    /// Postgres connection string.
    pub database_url: String,
    /// Engine TCP address.
    pub engine_addr: SocketAddr,
    /// HS256 JWT secret.
    pub jwt_secret: String,
    /// Access-token TTL.
    pub access_token_ttl: Duration,
    /// MFA challenge token TTL.
    pub mfa_token_ttl: Duration,
    /// Refresh-token TTL.
    pub refresh_token_ttl: Duration,
}

/// Deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    /// Local development.
    Development,
    /// Test runs.
    Test,
    /// Production-like deployment.
    Production,
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
    /// Loads configuration from process environment.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if required values are missing or malformed.
    pub fn from_env() -> Result<Self, ConfigError> {
        let app_env = parse_app_env(optional("APP_ENV").as_deref().unwrap_or("development"))?;
        let api_host = optional("API_HOST").unwrap_or_else(|| "127.0.0.1".to_owned());
        let api_port = parse_u16(
            "API_PORT",
            optional("API_PORT").as_deref().unwrap_or("8080"),
        )?;
        let engine_host = optional("ENGINE_HOST").unwrap_or_else(|| "127.0.0.1".to_owned());
        let engine_port = parse_u16(
            "ENGINE_PORT",
            optional("ENGINE_PORT").as_deref().unwrap_or("7878"),
        )?;
        let jwt_secret = required("JWT_SECRET")?;
        if app_env == AppEnv::Production && jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid {
                name: "JWT_SECRET",
                reason: "must be at least 32 bytes in production".to_owned(),
            });
        }

        Ok(Self {
            app_env,
            bind_addr: SocketAddr::new(parse_ip("API_HOST", &api_host)?, api_port),
            database_url: required("DATABASE_URL")?,
            engine_addr: SocketAddr::new(parse_ip("ENGINE_HOST", &engine_host)?, engine_port),
            jwt_secret,
            access_token_ttl: Duration::from_secs(15 * 60),
            mfa_token_ttl: Duration::from_secs(5 * 60),
            refresh_token_ttl: Duration::from_secs(30 * 24 * 60 * 60),
        })
    }
}

fn optional(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    optional(name).ok_or(ConfigError::Missing(name))
}

fn parse_app_env(value: &str) -> Result<AppEnv, ConfigError> {
    match value {
        "development" => Ok(AppEnv::Development),
        "test" => Ok(AppEnv::Test),
        "production" => Ok(AppEnv::Production),
        other => Err(ConfigError::Invalid {
            name: "APP_ENV",
            reason: format!("unsupported value {other}"),
        }),
    }
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

fn parse_u16(name: &'static str, value: &str) -> Result<u16, ConfigError> {
    value.parse::<u16>().map_err(|error| ConfigError::Invalid {
        name,
        reason: error.to_string(),
    })
}
