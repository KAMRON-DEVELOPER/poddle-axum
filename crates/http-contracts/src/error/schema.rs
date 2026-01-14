use serde::Serialize;

/// Serializable error for API responses
#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}
