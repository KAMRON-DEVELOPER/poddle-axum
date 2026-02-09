#[derive(Debug)]
pub enum TimeRangeError {
    StartAfterEnd,
    StartInFuture,
    EndInFuture,
    TimestampConversion,
}

impl std::fmt::Display for TimeRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartAfterEnd => write!(f, "Start time must be before end time"),
            Self::StartInFuture => write!(f, "Start time cannot be in the future"),
            Self::EndInFuture => write!(f, "End time cannot be in the future"),
            Self::TimestampConversion => write!(f, "Failed to convert timestamp to nanoseconds"),
        }
    }
}
