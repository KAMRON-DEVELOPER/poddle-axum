use std::{net::SocketAddr, path::PathBuf};

use config::{ConfigBuilder, ConfigError, Environment, File, builder::AsyncState};
use factory::factories::{
    amqp::AmqpConfig, database::DatabaseConfig, mailtrap::MailtrapConfig, redis::RedisConfig,
};
use serde::Deserialize;
use users_core::jwt::JwtConfig;

use crate::services::{
    github_oauth::GithubOAuthServiceConfig, google_oauth::GoogleOAuthServiceConfig,
    s3::S3ServiceConfig,
};

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server_address: SocketAddr,
    pub frontend_endpoint: String,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub amqp: AmqpConfig,
    pub kafka_bootstrap_servers: String,
    pub google_oauth: GoogleOAuthServiceConfig,
    pub github_oauth: GithubOAuthServiceConfig,
    pub s3: S3ServiceConfig,
    pub cookie_key: String,
    pub cookie_secure: bool,
    pub jwt: JwtConfig,
    pub mailtrap: MailtrapConfig,
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
