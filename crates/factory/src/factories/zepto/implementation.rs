use reqwest::Client;
use tracing::{debug, error};

use crate::factories::zepto::{
    EmailAddress, Payload, Recipient, ZeptoApiError, ZeptoApiResponse, ZeptoMail, error::ZeptoError,
};

use std::fmt;

impl fmt::Display for ZeptoApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {} ({:?})", self.code, self.message, self.details)
    }
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
        to_email: &str,
        name: &str,
        link: &str,
        email_service_api_key: &str,
    ) -> Result<(), ZeptoError> {
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
                    name: name.to_string(),
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
            .await?;

        let _ = res.status();
        let text = res.text().await?;

        let api_response = serde_json::from_str::<ZeptoApiResponse>(&text)?;

        match api_response {
            ZeptoApiResponse::Success(body) => {
                debug!("Zepto success: {:?}", body);
                Ok(())
            }
            ZeptoApiResponse::Failure { error } => {
                error!("Zepto error: {:?}", error);
                Err(ZeptoError::Api { error })
            }
        }
    }
}
