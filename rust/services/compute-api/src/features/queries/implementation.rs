use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::features::queries::{
    DeploymentMetricsQuery, DeploymentsMetricsQuery, LogQuery, TailQuery, error::TimeRangeError,
};

impl std::error::Error for TimeRangeError {}

impl DeploymentMetricsQuery {
    /// Calculate number of snapshots that fit in the time window
    pub fn snapshot_count(&self, scrape_interval: i64) -> isize {
        let minutes = self.minutes.max(5);
        let duration_secs = minutes * 60;
        ((duration_secs / scrape_interval).max(1)) as isize
    }
}

impl DeploymentsMetricsQuery {
    /// Calculate number of snapshots that fit in the time window
    pub fn snapshot_count(&self, scrape_interval: i64) -> isize {
        let minutes = self.minutes.max(5);
        let duration_secs = minutes * 60;
        ((duration_secs / scrape_interval).max(1)) as isize
    }
}

impl LogQuery {
    /// Validates and returns (start, end) timestamps
    pub fn resolve(&self) -> Result<(DateTime<Utc>, DateTime<Utc>), TimeRangeError> {
        let now = Utc::now();

        match (self.start, self.end) {
            // Both provided
            (Some(start), Some(end)) => {
                if start >= end {
                    return Err(TimeRangeError::StartAfterEnd);
                }
                if end > now {
                    return Err(TimeRangeError::EndInFuture);
                }
                Ok((start, end))
            }
            // Only start provided - use now as end
            (Some(start), None) => {
                if start >= now {
                    return Err(TimeRangeError::StartInFuture);
                }
                Ok((start, now))
            }
            // Only end provided - calculate start from minutes
            (None, Some(end)) => {
                if end > now {
                    return Err(TimeRangeError::EndInFuture);
                }
                let minutes = self.minutes.max(1);
                let start = end - Duration::minutes(minutes);
                Ok((start, end))
            }
            // Neither provided - use relative window
            (None, None) => {
                let minutes = self.minutes.max(1);
                let start = now - Duration::minutes(minutes);
                Ok((start, now))
            }
        }
    }

    /// Returns (start_nanos, end_nanos) as strings for Loki query
    /// Compatible with Loki's Unix nanosecond timestamps
    pub fn resolve_nanos(&self) -> Result<(String, String), TimeRangeError> {
        let (start, end) = self.resolve()?;
        let start_nanos = start
            .timestamp_nanos_opt()
            .ok_or(TimeRangeError::TimestampConversion)?;
        let end_nanos = end
            .timestamp_nanos_opt()
            .ok_or(TimeRangeError::TimestampConversion)?;

        Ok((start_nanos.to_string(), end_nanos.to_string()))
    }
}

impl TailQuery {
    /// Returns start timestamp in nanoseconds as string for Loki tail query
    pub fn resolve_nanos(&self) -> Result<String, TimeRangeError> {
        let now = Utc::now();
        let start = Utc.timestamp_nanos(self.start);

        if start >= now {
            return Err(TimeRangeError::StartInFuture);
        }

        let start_nanos = start
            .timestamp_nanos_opt()
            .ok_or(TimeRangeError::TimestampConversion)?;

        Ok(start_nanos.to_string())
    }
}
