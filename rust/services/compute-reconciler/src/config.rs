use std::{net::SocketAddr, path::PathBuf};

use compute_core::configs::PrometheusConfig;
use config::{ConfigBuilder, ConfigError, Environment, File, builder::AsyncState};
use factory::factories::{
    amqp::AmqpConfig, database::DatabaseConfig, observability::ObservabilityConfig,
    redis::RedisConfig,
};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server_address: SocketAddr,
    pub observability: ObservabilityConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub amqp: AmqpConfig,
    pub prometheus: PrometheusConfig,
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
