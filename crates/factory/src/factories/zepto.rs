use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::utilities::errors::AppError;
use tracing::debug;

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
pub struct ZeptoError {
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
    Failure { error: ZeptoError },
}

// ---------------------------------- ZeptoMail ----------------------------------

pub struct ZeptoMail {
    api_url: String,
    client: Client,
}

impl Default for ZeptoMail {
    fn default() -> Self {
        Self::new()
    }
}

impl ZeptoMail {
    pub fn new() -> Self {
        Self {
            api_url: "https://api.zeptomail.com/v1.1/email/template".to_string(),
            client: Client::new(),
        }
    }

    pub async fn send_email_verification_link(
        &self,
        to_email: String,
        name: String,
        link: String,
        email_service_api_key: String,
    ) -> Result<(), AppError> {
        debug!("Sending email 1");
        let payload = Payload {
            template_alias: "poddle-email-verification-link-key-alias".to_string(),
            from: EmailAddress {
                name: "Poddle Verification".to_string(),
                address: "verification@kronk.uz".to_string(),
            },
            to: vec![Recipient {
                email_address: EmailAddress {
                    address: to_email.to_string(),
                    name: name.clone(),
                },
            }],
            merge_info: serde_json::json!({
                "link": link
            }),
        };

        debug!("Sending email to '{}' with email '{}'", name, to_email);

        let res = self
            .client
            .post(&self.api_url)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("authorization", email_service_api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::ZeptoServiceError(format!("ZeptoMail request failed: {}", e)))?;

        let _ = res.status();
        let text = res.text().await?;

        match serde_json::from_str::<ZeptoApiResponse>(&text) {
            Ok(ZeptoApiResponse::Success(body)) => {
                debug!("Zepto success: {:?}", body);
                Ok(())
            }
            Ok(ZeptoApiResponse::Failure { error }) => {
                debug!("Zepto error: {:?}", error);
                Err(AppError::ZeptoServiceError(format!(
                    "ZeptoMail error: {} - {} ({:?})",
                    error.code, error.message, error.details
                )))
            }
            Err(err) => {
                debug!("Failed to parse ZeptoMail response: {:?}", err);
                Err(AppError::ZeptoServiceError(format!(
                    "Unexpected ZeptoMail response: {}",
                    err
                )))
            }
        }
    }
}
