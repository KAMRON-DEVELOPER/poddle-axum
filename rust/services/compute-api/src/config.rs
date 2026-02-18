use std::{net::SocketAddr, path::PathBuf};

use compute_core::{configs::PrometheusConfig, github_app::GithubAppConfig};
use config::{ConfigBuilder, ConfigError, Environment, File, builder::AsyncState};
use factory::factories::{
    amqp::AmqpConfig, database::DatabaseConfig, observability::ObservabilityConfig,
    redis::RedisConfig,
};
use serde::Deserialize;
use users_core::jwt::JwtConfig;

#[derive(Deserialize, Clone, Debug)]
pub struct LokiConfig {
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TempoConfig {
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server_address: SocketAddr,
    pub frontend_endpoint: String,
    pub observability: ObservabilityConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub amqp: AmqpConfig,
    pub prometheus: PrometheusConfig,
    pub cookie_key: String,
    pub jwt: JwtConfig,
    pub loki: LokiConfig,
    pub tempo: TempoConfig,
    pub github_app: GithubAppConfig,
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
