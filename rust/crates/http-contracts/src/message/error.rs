use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ErrorMessageResponse {
    pub message: String,
}
