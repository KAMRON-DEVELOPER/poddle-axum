use crate::error::AppError;

use compute_core::services::event_emission_service::error::EventEmissionServiceError;

impl From<EventEmissionServiceError> for AppError {
    fn from(e: EventEmissionServiceError) -> Self {
        match e {
            EventEmissionServiceError::SqlxError(error) => AppError::SqlxError(error),
            EventEmissionServiceError::RedisError(error) => AppError::RedisError(error),
        }
    }
}

impl From<lapin::Error> for AppError {
    fn from(value: lapin::Error) -> Self {
        match value {
            _ => AppError::InternalServerError(value.to_string()),
        }
    }
}
