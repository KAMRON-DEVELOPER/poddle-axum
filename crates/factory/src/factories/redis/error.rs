use thiserror::Error;

#[derive(Error, Debug)]
#[error("Redis error: {0}")]
pub struct RedisError(#[from] redis::RedisError);
