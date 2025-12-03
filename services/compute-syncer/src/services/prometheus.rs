use prometheus_http_query::Client;
use reqwest::Client as HttpClient;
use shared::utilities::{config::Config, errors::AppError};
use tracing::info;

#[derive(Clone)]
pub struct Prometheus {
    pub client: Client,
}

impl Prometheus {
    pub async fn new(config: &Config, http_client: HttpClient) -> Result<Self, AppError> {
        let client = Client::from(http_client, &config.prometheus_url)?;

        let ping_query = "up";

        match client.query(ping_query).get().await {
            Ok(_) => {
                info!(
                    "✅ Prometheus connection verified at {}",
                    config.prometheus_url
                );
            }
            Err(e) => {
                return Err(AppError::InternalError(format!(
                    "Prometheus connectivity check failed: {}",
                    e
                )));
            }
        }

        Ok(Self { client })
    }
}
