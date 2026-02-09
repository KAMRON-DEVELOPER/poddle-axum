use chrono::{TimeZone, Utc};

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
    /// Returns (start_nanos, end_nanos) as strings for Loki query
    /// Compatible with Loki's Unix nanosecond timestamps
    pub fn resolve_nanos(&self) -> Result<(String, Option<String>), TimeRangeError> {
        let now = Utc::now();

        if self.start > now {
            return Err(TimeRangeError::StartInFuture);
        }

        match self.end {
            Some(end) => {
                if self.start >= end {
                    return Err(TimeRangeError::StartAfterEnd);
                }
                if end > now {
                    return Err(TimeRangeError::EndInFuture);
                }

                Ok((
                    self.start
                        .timestamp_nanos_opt()
                        .ok_or(TimeRangeError::TimestampConversion)?
                        .to_string(),
                    Some(
                        end.timestamp_nanos_opt()
                            .ok_or(TimeRangeError::TimestampConversion)?
                            .to_string(),
                    ),
                ))
            }
            None => Ok((
                self.start
                    .timestamp_nanos_opt()
                    .ok_or(TimeRangeError::TimestampConversion)?
                    .to_string(),
                None,
            )),
        }
    }
}

impl TailQuery {
    /// Returns start timestamp in nanoseconds as string for Loki tail query
    pub fn resolve_nanos(&self) -> Result<String, TimeRangeError> {
        let now = Utc::now();
        let start = match self.start {
            Some(ns) => Utc.timestamp_nanos(ns),
            None => now,
        };

        if start > now {
            return Err(TimeRangeError::StartInFuture);
        }

        Ok(start
            .timestamp_nanos_opt()
            .ok_or(TimeRangeError::TimestampConversion)?
            .to_string())
    }
}
