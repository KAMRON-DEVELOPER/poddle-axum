use thiserror::Error;

use crate::factories::mailtrap::ErrorResponse;

#[derive(Error, Debug)]
pub enum MailtrapError {
    #[error("HTTP request failed")]
    Request(#[from] reqwest::Error),

    #[error("Failed to deserialize Mailtrap response")]
    Deserialization(#[from] serde_json::Error),

    #[error("ZeptoMail API error: {error}")]
    Api { error: ErrorResponse },
}
