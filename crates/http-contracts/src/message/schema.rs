use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
}
