use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::engine_client::{EngineClient, EngineClientError};

/// Shared readiness checker.
pub type SharedReadiness = Arc<dyn ReadinessCheck>;

/// Readiness checker.
#[async_trait]
pub trait ReadinessCheck: Send + Sync {
    /// Checks whether dependencies are ready.
    async fn check(&self, request_id: Uuid) -> Result<(), ReadinessError>;
}

/// Readiness failure.
#[derive(Debug, Error)]
pub enum ReadinessError {
    /// Database is not ready.
    #[error("database not ready")]
    Database,
    /// Engine is not ready.
    #[error("engine not ready")]
    Engine,
}

/// Production readiness checker for Postgres and engine TCP.
pub struct PgEngineReadiness {
    pool: PgPool,
    engine: EngineClient,
}

impl PgEngineReadiness {
    /// Creates a readiness checker.
    #[must_use]
    pub const fn new(pool: PgPool, engine: EngineClient) -> Self {
        Self { pool, engine }
    }
}

#[async_trait]
impl ReadinessCheck for PgEngineReadiness {
    async fn check(&self, request_id: Uuid) -> Result<(), ReadinessError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "api.ready.database_failed");
                ReadinessError::Database
            })?;
        self.engine
            .ping(request_id)
            .await
            .map_err(|error| map_engine_error(&error))?;
        Ok(())
    }
}

fn map_engine_error(error: &EngineClientError) -> ReadinessError {
    tracing::error!(error = %error, "api.ready.engine_failed");
    ReadinessError::Engine
}

#[cfg(test)]
pub(crate) mod tests {
    use async_trait::async_trait;
    use uuid::Uuid;

    use super::{ReadinessCheck, ReadinessError};

    #[derive(Debug)]
    pub struct StaticReadiness {
        ready: bool,
    }

    impl StaticReadiness {
        pub const fn ready() -> Self {
            Self { ready: true }
        }
    }

    #[async_trait]
    impl ReadinessCheck for StaticReadiness {
        async fn check(&self, _request_id: Uuid) -> Result<(), ReadinessError> {
            if self.ready {
                Ok(())
            } else {
                Err(ReadinessError::Database)
            }
        }
    }
}
