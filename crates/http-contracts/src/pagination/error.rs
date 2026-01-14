use crate::error::schema::ErrorResponse;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Pagination-specific errors
#[derive(Error, Debug)]
pub enum PaginationError {
    #[error("Offset must be positive")]
    NegativeOffset,

    #[error("Limit must be positive")]
    ZeroOrNegativeLimit,

    #[error("Limit cannot exceed 100")]
    LimitTooLarge,
}

/// Implement IntoResponse so it can be returned from an Axum handler
impl IntoResponse for PaginationError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::NegativeOffset => (StatusCode::BAD_REQUEST, "Offset cannot be negative"),
            Self::ZeroOrNegativeLimit => {
                (StatusCode::BAD_REQUEST, "Limit cannot be zero or negative")
            }
            Self::LimitTooLarge => (StatusCode::BAD_REQUEST, "Limit is too large"),
        };

        let body = Json(ErrorResponse {
            error: msg.to_string(),
        });

        (status, body).into_response()
    }
}
