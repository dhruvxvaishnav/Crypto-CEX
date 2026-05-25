use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

/// Clock port used by the server adapter.
pub trait Clock: Send + Sync {
    /// Returns the current UTC time as an RFC3339 timestamp.
    fn now_rfc3339(&self) -> String;
}

/// ID-source port used by the server adapter.
pub trait IdSource: Send + Sync {
    /// Returns a fresh UUID v4.
    fn next_uuid(&self) -> Uuid;
}

/// System-backed clock for production.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        format_unix_millis(duration.as_secs(), duration.subsec_millis())
    }
}

/// UUID v4 source for production.
#[derive(Debug, Default)]
pub struct UuidV4Source;

impl IdSource for UuidV4Source {
    fn next_uuid(&self) -> Uuid {
        Uuid::new_v4()
    }
}

fn format_unix_millis(seconds: u64, millis: u32) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let seconds_of_day = seconds % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_from_days(days);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

// Howard Hinnant's civil-from-days algorithm, adapted to signed integer math.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let adjusted_year = year + i64::from(month <= 2);

    (
        i32::try_from(adjusted_year).unwrap_or(i32::MAX),
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::format_unix_millis;

    #[test]
    fn formats_unix_epoch() {
        assert_eq!(format_unix_millis(0, 0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn formats_recent_utc_timestamp() {
        assert_eq!(
            format_unix_millis(1_735_689_600, 123),
            "2025-01-01T00:00:00.123Z"
        );
    }
}
