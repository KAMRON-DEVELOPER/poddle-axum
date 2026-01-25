use prometheus_http_query::Client;
use reqwest::Client as HttpClient;
use tracing::info;

use crate::error::AppError;

#[derive(Clone)]
pub struct Prometheus {
    pub client: Client,
}

impl Prometheus {
    pub async fn new(url: &str, http_client: HttpClient) -> Result<Self, AppError> {
        let client = Client::from(http_client, url)?;

        match client.query("up").get().await {
            Ok(_) => info!("✅ Successfully connected to Prometheus!"),
            Err(e) => {
                return Err(AppError::InternalServerError(format!(
                    "Failed to connect to Prometheus: {}. Check URL and credentials.",
                    e
                )));
            }
        }

        Ok(Self { client })
    }
}
