pub mod error;
pub mod implementation;

use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct Mailtrap {
    api_url: String,
    client: Client,
}

#[derive(Serialize)]
pub struct Mailbox {
    pub name: String,
    pub email: String,
}

#[derive(Serialize)]
pub struct Payload {
    pub from: Mailbox,
    pub to: Vec<Mailbox>,
    pub template_uuid: String,
    pub template_variables: serde_json::Value,
}

/// 200, Success. Message has been delivered.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct SuccessResponse {
    success: bool,
    message_ids: Vec<String>,
}

/// 400, Bad request. Fix errors listed in response before retrying.
/// 401, Unauthorized. Make sure you are sending correct credentials with the request before retrying.
/// 403, Forbidden. Make sure domain verification process is completed.
// 500, Internal error. Mail was not delivered. Retry later or contact support.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ErrorResponse {
    success: bool,
    errors: Vec<String>,
}

pub enum MailtrapApiResponse {
    Success(SuccessResponse),
    Error(ErrorResponse),
}
