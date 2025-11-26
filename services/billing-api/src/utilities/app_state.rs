use axum::extract::FromRef;
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
    pub http_client: Client,
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

impl FromRef<AppState> for Client {
    fn from_ref(state: &AppState) -> Self {
        state.http_client.clone()
    }
}
