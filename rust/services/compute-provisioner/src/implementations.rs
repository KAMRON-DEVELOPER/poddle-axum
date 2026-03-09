use compute_core::services::event_emission_service::error::EventEmissionServiceError;

use crate::error::AppError;

impl From<EventEmissionServiceError> for AppError {
    fn from(e: EventEmissionServiceError) -> Self {
        match e {
            EventEmissionServiceError::SqlxError(error) => AppError::SqlxError(error),
            EventEmissionServiceError::RedisError(error) => AppError::RedisError(error),
        }
    }
}
