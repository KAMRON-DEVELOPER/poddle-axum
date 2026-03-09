use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventEmissionServiceError {
    #[error("Database query error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
}
