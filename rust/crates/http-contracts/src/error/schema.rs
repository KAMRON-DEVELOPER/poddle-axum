use schemars::JsonSchema;
use serde::Serialize;

/// Serializable error for API responses
#[derive(Serialize, JsonSchema, Debug)]
pub struct ErrorResponse {
    pub error: String,
}
