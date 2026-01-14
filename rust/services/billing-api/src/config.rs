use std::{net::SocketAddr, path::PathBuf};

use sqlx::postgres::PgSslMode;
use tracing::Level;
use utility::get_config_value::get_config_value;
use utility::get_optional_config_value::get_optional_config_value;

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_address: SocketAddr,

    pub tracing_level: Level,

    // POSTGRES
    pub database_url: String,
    pub postgres_pool_size: Option<u32>,
    pub pg_ssl_mode: PgSslMode,

    // REDIS
    pub redis_url: Option<String>,
    pub redis_host: Option<String>,
    pub redis_port: Option<u16>,
    pub redis_username: Option<String>,
    pub redis_password: Option<String>,

    // RABBITMQ
    pub amqp_addr: String,

    // KAFKA BROKERS
    pub kafka_bootstrap_servers: String,

    pub cookie_key: String,

    // JWT
    pub jwt_secret_key: String,
    pub access_token_expire_in_minute: i64,
    pub refresh_token_expire_in_days: i64,
    pub email_verification_token_expire_in_hours: i64,
    pub refresh_token_renewal_threshold_days: i64,

    // EMAIL
    pub email_service_api_key: String,

    // SSL/TLS
    pub ca: Option<String>,
    pub ca_path: Option<PathBuf>,
    pub client_cert: Option<String>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key: Option<String>,
    pub client_key_path: Option<PathBuf>,

    // OBSERVABILITY
    pub otel_exporter_otlp_endpoint: String,
}

impl Config {
    pub async fn init(cargo_manifest_dir: PathBuf) -> Result<Self, AppError> {
        let socket_addr: SocketAddr = "0.0.0.0:8000"
            .parse()
            .expect("Invalid default socket address");
        let server_address = get_config_value(
            "SERVER_ADDRESS",
            Some("SERVER_ADDRESS"),
            None,
            Some(socket_addr),
        )
        .await;

        let tracing_level = get_config_value(
            "TRACING_LEVEL",
            Some("TRACING_LEVEL"),
            None,
            Some(Level::DEBUG),
        )
        .await;

        let postgres_url = get_config_value(
            "DATABASE_URL",
            Some("DATABASE_URL"),
            None,
            Some("postgresql://postgres:password@localhost:5432/poddle_db".to_string()),
        )
        .await;

        let postgres_pool_size =
            get_optional_config_value("DATABASE_POOL_SIZE", Some("DATABASE_POOL_SIZE"), None).await;

        let pg_ssl_mode =
            get_config_value("ssl_mode", Some("SSL_MODE"), None, Some(PgSslMode::Disable)).await;

        let redis_url = get_optional_config_value("REDIS_URL", Some("REDIS_URL"), None).await;
        let redis_host = get_optional_config_value("REDIS_HOST", Some("REDIS_HOST"), None).await;
        let redis_port = get_optional_config_value("REDIS_PORT", None, None).await;
        let redis_username =
            get_optional_config_value("REDIS_USERNAME", Some("REDIS_USERNAME"), None).await;
        let redis_password =
            get_optional_config_value("REDIS_PASSWORD", Some("REDIS_PASSWORD"), None).await;

        let amqp_addr = get_config_value(
            "AMQP_ADDR",
            Some("AMQP_ADDR"),
            None,
            Some("amqp://localhost:5672".to_string()),
        )
        .await;

        let kafka_bootstrap_servers = get_config_value(
            "KAFKA_BOOTSTRAP_SERVERS",
            Some("KAFKA_BOOTSTRAP_SERVERS"),
            None,
            Some("localhost:9092".to_string()),
        )
        .await;

        let cookie_key = get_config_value("COOKIE_KEY", Some("COOKIE_KEY"), None, None).await;

        let jwt_secret_key = get_config_value("SECRET_KEY", Some("SECRET_KEY"), None, None).await;
        let access_token_expire_in_minute = get_config_value(
            "ACCESS_TOKEN_EXPIRE_IN_MINUTE",
            Some("ACCESS_TOKEN_EXPIRE_IN_MINUTE"),
            None,
            None,
        )
        .await;
        let refresh_token_expire_in_days = get_config_value(
            "REFRESH_TOKEN_EXPIRE_IN_DAYS",
            Some("REFRESH_TOKEN_EXPIRE_IN_DAYS"),
            None,
            None,
        )
        .await;
        let email_verification_token_expire_in_hours = get_config_value(
            "EMAIL_VERIFICATION_TOKEN_EXPIRE_IN_HOURS",
            Some("EMAIL_VERIFICATION_TOKEN_EXPIRE_IN_HOURS"),
            None,
            None,
        )
        .await;
        let refresh_token_renewal_threshold_days = get_config_value(
            "REFRESH_TOKEN_RENEWAL_THRESHOLD_DAYS",
            Some("REFRESH_TOKEN_RENEWAL_THRESHOLD_DAYS"),
            None,
            None,
        )
        .await;

        let email_service_api_key = get_config_value(
            "EMAIL_SERVICE_API_KEY",
            Some("EMAIL_SERVICE_API_KEY"),
            None,
            None,
        )
        .await;

        // TLS certs: Docker secrets → fallback path
        let ca_path = cargo_manifest_dir.join("certs/ca/ca.pem");
        let ca = get_optional_config_value("ca.pem", Some("CA"), Some(&ca_path)).await;
        let client_cert_path = cargo_manifest_dir.join("certs/client/client-cert.pem");
        let client_cert = get_optional_config_value(
            "client-cert.pem",
            Some("CLIENT_CERT"),
            Some(&client_cert_path),
        )
        .await;
        let client_key_path = cargo_manifest_dir.join("certs/client/client-key.pem");
        let client_key =
            get_optional_config_value("client-key.pem", Some("CLIENT_KEY"), Some(&client_key_path))
                .await;

        let otel_exporter_otlp_endpoint = get_config_value(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            Some("OTEL_EXPORTER_OTLP_ENDPOINT"),
            None,
            Some("https://alloy-gateway.poddle.uz:4317".to_string()),
        )
        .await;

        let config = Config {
            server_address,
            tracing_level,
            database_url: postgres_url,
            postgres_pool_size,
            redis_url,
            redis_host,
            redis_port,
            redis_username,
            redis_password,
            amqp_addr,
            kafka_bootstrap_servers,
            cookie_key,
            jwt_secret_key,
            access_token_expire_in_minute,
            refresh_token_expire_in_days,
            email_verification_token_expire_in_hours,
            refresh_token_renewal_threshold_days,
            email_service_api_key,
            ca_path: Some(ca_path),
            ca,
            client_cert_path: Some(client_cert_path),
            client_cert,
            client_key_path: Some(client_key_path),
            client_key,
            pg_ssl_mode,
            otel_exporter_otlp_endpoint,
        };

        Ok(config)
    }
}
