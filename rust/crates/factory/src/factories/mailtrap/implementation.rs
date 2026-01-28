use reqwest::Client;
use tracing::{debug, error};

use crate::factories::mailtrap::{
    ErrorResponse, Mailbox, Mailtrap, MailtrapConfig, Payload, SuccessResponse,
    error::MailtrapError,
};

use std::fmt;

impl Default for Mailtrap {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailtrap {
    pub fn new() -> Self {
        Self {
            url: "https://send.api.mailtrap.io/api/send".to_string(),
            client: Client::new(),
        }
    }

    #[tracing::instrument(
        name = "mailtrip.send_email_verification_link",
        skip_all, fields(recipient = %to_email)
        err
    )]
    pub async fn send_email_verification_link(
        &self,
        to_name: &str,
        to_email: &str,
        link: &str,
        cfg: &MailtrapConfig,
    ) -> Result<(), MailtrapError> {
        debug!("Sending email...");

        let template = cfg.verification.clone();

        let payload = Payload {
            from: Mailbox {
                name: template.from_name,
                email: template.from_email,
            },
            to: vec![Mailbox {
                email: to_email.to_string(),
                name: to_name.to_string(),
            }],
            template_uuid: template.template_uuid,
            template_variables: serde_json::json!({
                "link": link
            }),
        };

        debug!("Sending email to '{}' with email '{}'", to_name, to_email);

        let res = self
            .client
            .post(&self.url)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("authorization", cfg.clone().api_key)
            .json(&payload)
            .send()
            .await?;

        let status_code = res.status();

        if status_code == 200 {
            let response = res.json::<SuccessResponse>().await?;
            debug!("Mailtrap success: {:?}", response);
            Ok(())
        } else {
            let response = res.json::<ErrorResponse>().await?;
            error!("Mailtrap error: {:?}", response);
            Err(MailtrapError::Api { error: response })
        }
    }

    #[tracing::instrument(name = "mailtrip.send_feedback_confirmation", skip_all, fields(recipient = %to_email), err)]
    pub async fn send_feedback_confirmation(
        &self,
        to_name: &str,
        to_email: &str,
        message: &str,
        cfg: &MailtrapConfig,
    ) -> Result<(), MailtrapError> {
        debug!("Sending email...");

        let template = cfg.feedback_confirmation.clone();

        let payload = Payload {
            from: Mailbox {
                name: template.from_name,
                email: template.from_email,
            },
            to: vec![Mailbox {
                email: to_email.to_string(),
                name: to_name.to_string(),
            }],
            template_uuid: template.template_uuid,
            template_variables: serde_json::json!({
                "name": to_name,
                "message": message
            }),
        };

        debug!("Sending email to '{}' with email '{}'", to_name, to_email);

        let res = self
            .client
            .post(&self.url)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("authorization", cfg.clone().api_key)
            .json(&payload)
            .send()
            .await?;

        let status_code = res.status();

        if status_code == 200 {
            let response = res.json::<SuccessResponse>().await?;
            debug!("Mailtrap success: {:?}", response);
            Ok(())
        } else {
            let response = res.json::<ErrorResponse>().await?;
            error!("Mailtrap error: {:?}", response);
            Err(MailtrapError::Api { error: response })
        }
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "success: {}, errors: {:?}", self.success, self.errors)
    }
}
