use thiserror::Error;

use crate::factories::zepto::ZeptoApiError;

#[derive(Error, Debug)]
pub enum ZeptoError {
    #[error("HTTP request to ZeptoMail failed")]
    Request(#[from] reqwest::Error),

    #[error("Failed to deserialize ZeptoMail response")]
    Deserialize(#[from] serde_json::Error),

    #[error("ZeptoMail API error: {error}")]
    Api { error: ZeptoApiError },
}
