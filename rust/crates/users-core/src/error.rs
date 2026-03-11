use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClaimsError {
    #[error("failed to generate authentication token")]
    Creation,

    #[error("expired token")]
    Expired,

    #[error("incorrect token type provided")]
    WrongType,

    #[error("invalid token")]
    Invalid,

    #[error("Failed to extract private key from state")]
    KeyError,
}

impl IntoResponse for ClaimsError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::Creation => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate authentication token. Please try again.",
            ),
            Self::Expired => (
                StatusCode::UNAUTHORIZED,
                "Authentication token has expired. Please login again.",
            ),
            Self::WrongType => (StatusCode::FORBIDDEN, "Incorrect token type provided"),
            Self::Invalid => (
                StatusCode::UNAUTHORIZED,
                "Invalid authentication token provided",
            ),
            Self::KeyError => (
                StatusCode::FORBIDDEN,
                "Failed to extract private key from state",
            ),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
