use crate::config::Config;
use crate::error::AppError;
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use factory::factories::{amqp::Amqp, database::Database, kafka::Kafka, redis::Redis};

use reqwest::Client;
use rustls::ClientConfig;
use users_core::jwt::JwtCapability;

#[derive(FromRef, Clone)]
pub struct AppState {
    pub rustls_config: Option<ClientConfig>,
    pub database: Database,
    pub redis: Redis,
    pub amqp: Amqp,
    pub kafka: Option<Kafka>,
    pub config: Config,
    pub http_client: Client,
    pub key: Key,
}

impl AppState {
    pub async fn init(cfg: &Config) -> Result<Self, AppError> {
        let database = Database::new(&cfg.database).await;
        let redis = Redis::new(&cfg.redis).await;
        let amqp = Amqp::new(&cfg.amqp).await;
        let http_client = reqwest::ClientBuilder::new()
            .build()
            .unwrap_or_else(|e| panic!("Failed to construct http client: {}", e));
        let key = Key::from(cfg.cookie_key.as_bytes());

        Ok(Self {
            rustls_config: None,
            database,
            redis,
            amqp,
            kafka: None,
            config: cfg.clone(),
            http_client,
            key,
        })
    }
}

// Option B: State can produce a JwtConfig via FromRef
impl FromRef<AppState> for Box<dyn JwtCapability> {
    fn from_ref(state: &AppState) -> Self {
        Box::new(state.config.clone()) // assuming Config implements JwtConfig
    }
}

// Option A: State itself implements JwtConfig
// impl JwtConfig for AppState {
//     fn jwt_secret(&self) -> &str {
//         self.config.jwt_secret_key.as_str()
//     }

//     fn access_token_expire_in_minute(&self) -> i64 {
//         self.config.access_token_expire_in_minute
//     }

//     fn refresh_token_expire_in_days(&self) -> i64 {
//         self.config.refresh_token_expire_in_days
//     }

//     fn email_verification_token_expire_in_hours(&self) -> i64 {
//         self.config.email_verification_token_expire_in_hours
//     }
// }

// impl FromRef<AppState> for Option<ClientConfig> {
//     fn from_ref(state: &AppState) -> Self {
//         state.rustls_config.clone()
//     }
// }

// impl FromRef<AppState> for Database {
//     fn from_ref(state: &AppState) -> Self {
//         state.database.clone()
//     }
// }

// impl FromRef<AppState> for Redis {
//     fn from_ref(state: &AppState) -> Self {
//         state.redis.clone()
//     }
// }

// impl FromRef<AppState> for Amqp {
//     fn from_ref(state: &AppState) -> Self {
//         state.amqp.clone()
//     }
// }

// impl FromRef<AppState> for Option<Kafka> {
//     fn from_ref(state: &AppState) -> Self {
//         state.kafka.clone()
//     }
// }

// impl FromRef<AppState> for Config {
//     fn from_ref(state: &AppState) -> Self {
//         state.config.clone()
//     }
// }

// impl FromRef<AppState> for Key {
//     fn from_ref(state: &AppState) -> Self {
//         state.key.clone()
//     }
// }

// impl FromRef<AppState> for GoogleOAuthClient {
//     fn from_ref(state: &AppState) -> Self {
//         state.google_oauth_client.clone()
//     }
// }

// impl FromRef<AppState> for GithubOAuthClient {
//     fn from_ref(state: &AppState) -> Self {
//         state.github_oauth_client.clone()
//     }
// }

// impl FromRef<AppState> for Client {
//     fn from_ref(state: &AppState) -> Self {
//         state.http_client.clone()
//     }
// }

// impl FromRef<AppState> for AmazonS3 {
//     fn from_ref(state: &AppState) -> Self {
//         state.s3.clone()
//     }
// }

// impl FromRef<AppState> for GoogleCloudStorage {
//     fn from_ref(state: &AppState) -> Self {
//         state.gcs.clone()
//     }
// }
