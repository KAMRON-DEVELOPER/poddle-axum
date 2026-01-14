use thiserror::Error;

#[derive(Error, Debug)]
pub enum AmqpError {
    // #[error("Lapin error, {0}")]
    // LapinError(#[from] lapin::Error),
    #[error("Serde json error, {0}")]
    SerializationError(#[from] serde_json::Error),
}
