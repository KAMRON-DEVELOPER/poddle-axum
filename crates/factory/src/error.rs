use thiserror::Error;

use crate::factories::zepto::ZeptoApiError;

use std::fmt;

impl fmt::Display for ZeptoApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {} ({:?})", self.code, self.message, self.details)
    }
}

#[derive(Debug, Error)]
pub enum ZeptoError {
    #[error("HTTP request to ZeptoMail failed")]
    Request(#[from] reqwest::Error),

    #[error("Failed to deserialize ZeptoMail response")]
    Deserialize(#[from] serde_json::Error),

    #[error("ZeptoMail API error: {error}")]
    Api { error: ZeptoApiError },
}
