use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use time::OffsetDateTime;
use tokio::time::Instant;
use uuid::Uuid;

/// Clock port used by auth and middleware code.
pub trait Clock: Send + Sync {
    /// Returns the current UTC timestamp.
    fn now(&self) -> OffsetDateTime;
}

/// UUID source used by business services.
pub trait IdSource: Send + Sync {
    /// Returns a new UUID.
    fn new_uuid(&self) -> Uuid;
}

/// Delay port used for security response floors.
#[async_trait]
pub trait Delay: Send + Sync {
    /// Sleeps until at least `deadline`, if it is in the future.
    async fn sleep_until(&self, deadline: Instant);
}

/// Shared clock trait object.
pub type SharedClock = Arc<dyn Clock>;

/// Shared delay trait object.
pub type SharedDelay = Arc<dyn Delay>;

/// Shared ID-source trait object.
pub type SharedIds = Arc<dyn IdSource>;

/// Production clock backed by system time.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        OffsetDateTime::from_unix_timestamp(i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }
}

/// Production delay backed by Tokio timers.
#[derive(Debug, Default)]
pub struct TokioDelay;

#[async_trait]
impl Delay for TokioDelay {
    async fn sleep_until(&self, deadline: Instant) {
        tokio::time::sleep_until(deadline).await;
    }
}

/// Production UUID v4 source.
#[derive(Debug, Default)]
pub struct UuidSource;

impl IdSource for UuidSource {
    fn new_uuid(&self) -> Uuid {
        Uuid::new_v4()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use time::OffsetDateTime;
    use tokio::time::Instant;

    use uuid::Uuid;

    use super::{Clock, Delay, IdSource};

    #[derive(Debug)]
    pub struct FixedClock {
        now: OffsetDateTime,
    }

    impl FixedClock {
        pub fn new(now: OffsetDateTime) -> Arc<Self> {
            Arc::new(Self { now })
        }
    }

    impl Clock for FixedClock {
        fn now(&self) -> OffsetDateTime {
            self.now
        }
    }

    #[derive(Debug, Default)]
    pub struct NoopDelay;

    #[async_trait]
    impl Delay for NoopDelay {
        async fn sleep_until(&self, _deadline: Instant) {}
    }

    #[derive(Debug)]
    pub struct FixedIds {
        value: Uuid,
    }

    impl FixedIds {
        pub fn new(value: Uuid) -> Arc<Self> {
            Arc::new(Self { value })
        }
    }

    impl IdSource for FixedIds {
        fn new_uuid(&self) -> Uuid {
            self.value
        }
    }
}
