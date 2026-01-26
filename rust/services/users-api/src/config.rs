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
    pub rust_log: Option<String>,
    pub log_format: Option<String>,
    pub tracing_level: Option<String>,
    pub server_address: SocketAddr,
    pub frontend_endpoint: String,
    pub otel_exporter_otlp_endpoint: String,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub amqp: AmqpConfig,
    pub cookie_key: String,
    pub cookie_secure: bool,
    pub jwt: JwtConfig,
    pub google_oauth: GoogleOAuthServiceConfig,
    pub github_oauth: GithubOAuthServiceConfig,
    pub s3: S3ServiceConfig,
    pub mailtrap: MailtrapConfig,
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
