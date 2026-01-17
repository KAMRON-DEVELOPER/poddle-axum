pub mod error;
pub mod implementation;

use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
}
