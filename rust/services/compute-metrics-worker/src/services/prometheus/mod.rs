use compute_core::configs::PrometheusConfig;
use prometheus_http_query::Client;
use tracing::info;
pub mod implementations;
use crate::error::AppError;

#[derive(Clone)]
pub struct Prometheus {
    pub client: Client,
    pub cfg: PrometheusConfig,
}

impl Prometheus {
    pub async fn new(cfg: &PrometheusConfig) -> Result<Self, AppError> {
        let cfg = cfg.clone();

        let client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let client = Client::from(client, &cfg.url)?;

        client.query("up").get().await?;
        info!("✅ Successfully connected to Prometheus!");

        Ok(Self { client, cfg })
    }
}
