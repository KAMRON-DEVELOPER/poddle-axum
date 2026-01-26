use std::{net::SocketAddr, path::PathBuf};

use config::{ConfigBuilder, ConfigError, Environment, File, builder::AsyncState};
use factory::factories::{amqp::AmqpConfig, database::DatabaseConfig, redis::RedisConfig};
use serde::Deserialize;

use crate::services::{
    kubernetes_service::KubernetesServiceConfig, vault_service::VaultServiceConfig,
};

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub rust_log: Option<String>,
    pub tracing_level: Option<String>,
    pub server_address: SocketAddr,
    pub otel_exporter_otlp_endpoint: String,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub amqp_addr: AmqpConfig,
    pub kubernetes: KubernetesServiceConfig,
    pub vault: VaultServiceConfig,
}

impl Config {
    pub async fn init(path: PathBuf) -> Result<Self, ConfigError> {
        let cfg = ConfigBuilder::<AsyncState>::default()
            .add_source(File::from(path))
            .add_source(Environment::default())
            .build()
            .await?;

        cfg.try_deserialize()
    }
}
