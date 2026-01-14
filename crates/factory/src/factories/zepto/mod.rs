pub mod error;
pub mod implementation;

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ZeptoResponseData {
    code: String,
    message: String,
    #[serde(default)]
    additional_info: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ZeptoResponse {
    pub data: Vec<ZeptoResponseData>,
    pub message: String,
    pub request_id: String,
    pub object: String,
}

#[derive(Serialize)]
pub struct EmailAddress {
    pub name: String,
    pub address: String,
}

#[derive(Serialize)]
pub struct Recipient {
    pub email_address: EmailAddress,
}

#[derive(Serialize)]
pub struct Payload {
    pub template_alias: String,
    pub from: EmailAddress,
    pub to: Vec<Recipient>,
    pub merge_info: serde_json::Value,
}

#[derive(Deserialize, Debug)]
pub struct ZeptoErrorDetail {
    pub code: String,
    #[serde(default)]
    pub target_value: Option<String>,
    pub message: String,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ZeptoApiError {
    pub code: String,
    pub message: String,
    pub request_id: String,
    #[serde(default)]
    pub details: Vec<ZeptoErrorDetail>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ZeptoApiResponse {
    Success(ZeptoResponse),
    Failure { error: ZeptoApiError },
}

pub struct ZeptoMail {
    api_url: String,
    client: Client,
}
