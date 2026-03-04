pub mod error;
pub mod implementation;

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, Debug)]
pub struct MessageResponse {
    pub message: String,
}
