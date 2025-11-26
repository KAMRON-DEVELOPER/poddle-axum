use crate::services::build_oauth::{GithubOAuthClient, GoogleOAuthClient};
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use object_store::{aws::AmazonS3, gcp::GoogleCloudStorage};
use reqwest::Client;
use rustls::ClientConfig;
use shared::{
    services::{amqp::Amqp, database::Database, kafka::Kafka, redis::Redis},
    utilities::config::Config,
};

#[derive(Clone)]
pub struct AppState {
    pub rustls_config: Option<ClientConfig>,
    pub database: Database,
    pub redis: Redis,
    pub amqp: Amqp,
    pub kafka: Kafka,
    pub config: Config,
    pub key: Key,
    pub google_oauth_client: GoogleOAuthClient,
    pub github_oauth_client: GithubOAuthClient,
    pub http_client: Client,
    pub s3: AmazonS3,
    pub gcs: GoogleCloudStorage,
}

impl FromRef<AppState> for Option<ClientConfig> {
    fn from_ref(state: &AppState) -> Self {
        state.rustls_config.clone()
    }
}

impl FromRef<AppState> for Database {
    fn from_ref(state: &AppState) -> Self {
        state.database.clone()
    }
}

impl FromRef<AppState> for Redis {
    fn from_ref(state: &AppState) -> Self {
        state.redis.clone()
    }
}

impl FromRef<AppState> for Amqp {
    fn from_ref(state: &AppState) -> Self {
        state.amqp.clone()
    }
}

impl FromRef<AppState> for Kafka {
    fn from_ref(state: &AppState) -> Self {
        state.kafka.clone()
    }
}

impl FromRef<AppState> for Config {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

impl FromRef<AppState> for GoogleOAuthClient {
    fn from_ref(state: &AppState) -> Self {
        state.google_oauth_client.clone()
    }
}

impl FromRef<AppState> for GithubOAuthClient {
    fn from_ref(state: &AppState) -> Self {
        state.github_oauth_client.clone()
    }
}

impl FromRef<AppState> for Client {
    fn from_ref(state: &AppState) -> Self {
        state.http_client.clone()
    }
}

impl FromRef<AppState> for AmazonS3 {
    fn from_ref(state: &AppState) -> Self {
        state.s3.clone()
    }
}

impl FromRef<AppState> for GoogleCloudStorage {
    fn from_ref(state: &AppState) -> Self {
        state.gcs.clone()
    }
}
