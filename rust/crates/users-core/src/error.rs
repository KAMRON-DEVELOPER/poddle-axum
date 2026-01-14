use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum JwtError {
    #[error("failed to generate authentication token")]
    Creation,

    #[error("invalid token")]
    Invalid,

    #[error("expired token")]
    Expired,

    #[error("incorrect token type provided")]
    WrongType,
}

impl IntoResponse for JwtError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::Creation => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate authentication token. Please try again.",
            ),
            Self::Invalid => (StatusCode::UNAUTHORIZED, "Invalid authentication token provided"),
            Self::Expired => (StatusCode::UNAUTHORIZED, "Authentication token has expired. Please login again."),
            Self::WrongType => (
                StatusCode::FORBIDDEN,
                "Incorrect token type provided",
            ),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
