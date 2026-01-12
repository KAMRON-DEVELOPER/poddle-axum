use crate::{
    config::Config,
    services::{
        build_oauth::{
            GithubOAuthClient, GoogleOAuthClient, build_github_oauth_client,
            build_google_oauth_client,
        },
        build_s3::{build_gcs, build_s3},
    },
};
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use factory::factories::{amqp::Amqp, database::Database, kafka::Kafka, redis::Redis};
use object_store::{aws::AmazonS3, gcp::GoogleCloudStorage};
use reqwest::Client;
use rustls::ClientConfig;
use crate::error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub rustls_config: Option<ClientConfig>,
    pub database: Database,
    pub redis: Redis,
    pub amqp: Amqp,
    pub kafka: Option<Kafka>,
    pub config: Config,
    pub key: Key,
    pub google_oauth_client: GoogleOAuthClient,
    pub github_oauth_client: GithubOAuthClient,
    pub http_client: Client,
    pub s3: AmazonS3,
    pub gcs: GoogleCloudStorage,
}

impl AppState {
    pub async fn init(config: &Config) -> Result<Self, AppError> {
        // let rustls_config = build_rustls_config(&config)?;
        let database = Database::new(config).await;
        let redis = Redis::new(config).await;
        let amqp = Amqp::new(config).await;
        // let kafka = Kafka::new(config, "users-service")?;
        let key = Key::from(config.cookie_key.as_bytes());
        let google_oauth_client = build_google_oauth_client(config);
        let github_oauth_client = build_github_oauth_client(config);
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build().unwrap_or_else(|e| {panic!("Couldn't construct http client: {}", e)});
        let s3 = build_s3(config);
        let gcs = build_gcs(config);

        Ok(Self {
            rustls_config: None,
            database,
            redis,
            amqp,
            kafka: None,
            config: config.clone(),
            key,
            google_oauth_client,
            github_oauth_client,
            http_client,
            s3,
            gcs,
        })
    }
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

impl FromRef<AppState> for Option<Kafka> {
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
