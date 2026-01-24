use std::path::PathBuf;

use config::{ConfigBuilder, ConfigError, Environment, File, builder::AsyncState};
use factory::factories::{
    amqp::AmqpConfig, database::DatabaseConfig, redis::RedisConfig, tls::TlsConfig,
};
use serde::Deserialize;

use crate::services::{
    kubernetes_service::KubernetesServiceConfig, vault_service::VaultServiceConfig,
};

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub tracing_level: String,
    pub server_address: String,
    pub kubernetes: KubernetesServiceConfig,
    pub vault: VaultServiceConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub amqp_addr: AmqpConfig,
    pub kafka_bootstrap_servers: String,
    pub tls: TlsConfig,
    pub otel_exporter_otlp_endpoint: String,
}

impl Config {
    pub async fn init(cargo_manifest_dir: PathBuf) -> Result<Self, ConfigError> {
        let cfg = ConfigBuilder::<AsyncState>::default()
            .add_source(File::from(cargo_manifest_dir.join("settings.json")))
            .add_source(Environment::default())
            .build()
            .await?;

        cfg.try_deserialize()
    }
}
