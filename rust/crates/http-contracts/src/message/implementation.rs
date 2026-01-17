use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::message::MessageResponse;

impl MessageResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl IntoResponse for MessageResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}
