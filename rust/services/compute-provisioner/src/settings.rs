use std::path::PathBuf;

use config::{Config, ConfigError, Environment, File};
use factory::factories::{database::DatabaseSettings, redis::RedisSettings, tls::TlsSettings};
use serde::Deserialize;

use crate::services::{kubernetes_service::KubernetesSettings, vault_service::VaultSettings};

#[derive(Deserialize, Clone, Debug)]
pub struct Settings {
    pub tracing_level: String,
    pub server_address: String,
    pub kubernetes: KubernetesSettings,
    pub vault: VaultSettings,
    pub database: DatabaseSettings,
    pub redis: RedisSettings,
    pub amqp_addr: String,
    pub kafka_bootstrap_servers: String,
    pub tls: TlsSettings,
    pub otel_exporter_otlp_endpoint: String,
}

impl Settings {
    pub async fn init(cargo_manifest_dir: PathBuf) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_async_source(File::from(cargo_manifest_dir.join("settings.json")))
            .add_async_source(Environment::default())
            .build()
            .await?;

        config.try_deserialize()
    }
}
