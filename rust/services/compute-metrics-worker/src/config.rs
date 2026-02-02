use std::{net::SocketAddr, path::PathBuf};

use config::{ConfigBuilder, ConfigError, Environment, File, builder::AsyncState};
use factory::factories::{observability::ObservabilityConfig, redis::RedisConfig};
use serde::Deserialize;
use compute_core::configs::PrometheusConfig;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server_address: SocketAddr,
    pub observability: ObservabilityConfig,
    pub redis: RedisConfig,
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
