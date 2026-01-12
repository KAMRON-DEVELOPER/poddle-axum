use std::{net::SocketAddr, path::PathBuf};

use factory::factories::tls::Tls;
use factory::factories::{amqp::AmqpConfig, database::DatabaseConfig, redis::RedisConfig};
use lapin::tcp::{OwnedIdentity, OwnedTLSConfig};
use redis::{
    ClientTlsConfig, ConnectionAddr, ConnectionInfo, IntoConnectionInfo, ProtocolVersion,
    RedisConnectionInfo, TlsCertificates,
};
use shared::utilities::{
    config::{get_config_value, get_optional_config_value},
    errors::AppError,
};
use sqlx::postgres::PgSslMode;
use tracing::{Level, info};
use users_core::jwt::JwtConfig;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_address: SocketAddr,
    pub frontend_endpoint: String,

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

    // GCP
    pub gcs_bucket_name: Option<String>,
    pub gcp_service_account: Option<String>,
    pub gcp_service_account_path: Option<PathBuf>,

    pub google_oauth_client_id: String,
    pub google_oauth_client_secret: String,
    pub google_oauth_redirect_url: String,

    pub github_oauth_client_id: String,
    pub github_oauth_client_secret: String,
    pub github_oauth_redirect_url: String,

    pub cookie_key: String,

    // S3
    pub s3_access_key_id: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_region: Option<String>,
    pub s3_bucket_name: Option<String>,

    // JWT
    pub jwt_secret_key: String,
    pub access_token_expire_in_minute: i64,
    pub refresh_token_expire_in_days: i64,
    pub email_verification_token_expire_in_hours: i64,
    pub refresh_token_renewal_threshold_days: i64,
    pub cookie_secure: bool,

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
            "SERVER_ADDRES",
            Some("SERVER_ADDRES"),
            None,
            Some(socket_addr),
        )
        .await;

        let frontend_endpoint = get_config_value(
            "FRONTEND_ENDPOINT",
            Some("FRONTEND_ENDPOINT"),
            None,
            Some("http://localhost:5173".to_string()),
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

        let gcs_bucket_name =
            get_optional_config_value("GCS_BUCKET_NAME", Some("GCS_BUCKET_NAME"), None).await;
        let gcp_service_account_path = cargo_manifest_dir.join("certs/service-account.json");
        let gcp_service_account = get_optional_config_value(
            "service_account.json",
            Some("SERVICE_ACCOUNT"),
            Some(&gcp_service_account_path),
        )
        .await;

        let google_oauth_client_id = get_config_value(
            "GOOGLE_OAUTH_CLIENT_ID",
            Some("GOOGLE_OAUTH_CLIENT_ID"),
            None,
            None,
        )
        .await;
        let google_oauth_client_secret = get_config_value(
            "GOOGLE_OAUTH_CLIENT_SECRET",
            Some("GOOGLE_OAUTH_CLIENT_SECRET"),
            None,
            None,
        )
        .await;
        let google_oauth_redirect_url = get_config_value(
            "GOOGLE_OAUTH_REDIRECT_URL",
            Some("GOOGLE_OAUTH_REDIRECT_URL"),
            None,
            None,
        )
        .await;

        let github_oauth_client_id = get_config_value(
            "GITHUB_OAUTH_CLIENT_ID",
            Some("GITHUB_OAUTH_CLIENT_ID"),
            None,
            None,
        )
        .await;
        let github_oauth_client_secret = get_config_value(
            "GITHUB_OAUTH_CLIENT_SECRET",
            Some("GITHUB_OAUTH_CLIENT_SECRET"),
            None,
            None,
        )
        .await;
        let github_oauth_redirect_url = get_config_value(
            "GITHUB_OAUTH_REDIRECT_URL",
            Some("GITHUB_OAUTH_REDIRECT_URL"),
            None,
            None,
        )
        .await;

        let cookie_key = get_config_value("COOKIE_KEY", Some("COOKIE_KEY"), None, None).await;
        let cookie_secure =
            get_config_value("COOKIE_SECURE", Some("COOKIE_SECURE"), None, None).await;

        let s3_access_key_id =
            get_optional_config_value("S3_ACCESS_KEY_ID", Some("S3_ACCESS_KEY_ID"), None).await;
        let s3_secret_key =
            get_optional_config_value("S3_SECRET_KEY", Some("S3_SECRET_KEY"), None).await;
        let s3_endpoint = get_optional_config_value("S3_ENDPOINT", Some("S3_ENDPOINT"), None).await;
        let s3_region = get_optional_config_value("S3_REGION", Some("S3_REGION"), None).await;
        let s3_bucket_name =
            get_optional_config_value("S3_BUCKET_NAME", Some("S3_BUCKET_NAME"), None).await;
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
            frontend_endpoint,
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
            gcs_bucket_name,
            gcp_service_account,
            gcp_service_account_path: Some(gcp_service_account_path),
            google_oauth_client_id,
            google_oauth_client_secret,
            google_oauth_redirect_url,
            github_oauth_client_id,
            github_oauth_client_secret,
            github_oauth_redirect_url,
            cookie_key,
            s3_access_key_id,
            s3_secret_key,
            s3_endpoint,
            s3_region,
            s3_bucket_name,
            jwt_secret_key,
            access_token_expire_in_minute,
            refresh_token_expire_in_days,
            email_verification_token_expire_in_hours,
            refresh_token_renewal_threshold_days,
            cookie_secure,
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

// Map your local config to the Factory's needs
impl DatabaseConfig for Config {
    type Tls = Tls;

    fn database_url(&self) -> String {
        self.database_url.clone()
    }
    fn max_connections(&self) -> u32 {
        self.postgres_pool_size.unwrap_or_default()
    }
    fn pg_ssl_mode(&self) -> PgSslMode {
        self.pg_ssl_mode
    }
    fn tls_config(&self) -> Self::Tls {
        Tls {
            ca: self.ca.clone(),
            ca_path: self.ca_path.clone(),
            client_cert: self.client_cert.clone(),
            client_cert_path: self.client_cert_path.clone(),
            client_key: self.client_key.clone(),
            client_key_path: self.client_key_path.clone(),
        }
    }
}

impl RedisConfig for Config {
    fn connection_info(&self) -> impl IntoConnectionInfo {
        // Prefer explicit host/port
        if let Some(host) = &self.redis_host
            && let Some(port) = self.redis_port
        {
            let conn_info = ConnectionInfo {
                addr: ConnectionAddr::Tcp(host.to_string(), port),
                redis: RedisConnectionInfo {
                    db: 0,
                    username: self.redis_username.clone(),
                    password: self.redis_password.clone(),
                    protocol: ProtocolVersion::RESP3,
                },
            };

            return conn_info;
        }

        // Fallback to URL
        let mut url = self
            .redis_url
            .clone()
            .expect("Either `REDIS_URL` or `REDIS_HOST`+`REDIS_PORT` must be set");

        // Enforce RESP3
        if !url.contains("protocol=") {
            let sep = if url.contains('?') { "&" } else { "?" };
            url.push_str(sep);
            url.push_str("protocol=resp3");
        }

        // Convert URL → ConnectionInfo
        let mut conn_info = url.into_connection_info().expect("Invalid Redis URL");

        // Ensure credentials override URL if explicitly provided
        if self.redis_username.is_some() || self.redis_password.is_some() {
            conn_info.redis.username = self.redis_username.clone();
            conn_info.redis.password = self.redis_password.clone();
        }

        conn_info
    }

    fn tls_certificates(&self) -> Option<TlsCertificates> {
        if let Some(ca) = &self.ca
            && let Some(client_cert) = &self.client_cert
            && let Some(client_key) = &self.client_key
        {
            // Structure to hold mTLS client certificate and key binaries in PEM format
            let client_tls_config = ClientTlsConfig {
                client_cert: client_cert.as_bytes().to_vec(),
                client_key: client_key.as_bytes().to_vec(),
            };

            // Structure to hold TLS certificates
            // * client_tls: binaries of clientkey and certificate within a ClientTlsConfig structure if mTLS is used
            // * root_cert: binary CA certificate in PEM format if CA is not in local truststore
            let tls_certs = TlsCertificates {
                client_tls: Some(client_tls_config),
                root_cert: Some(ca.as_bytes().to_vec()),
            };

            return Some(tls_certs);
        }

        None
    }
}

impl AmqpConfig for Config {
    fn uri(&self) -> String {
        self.amqp_addr.clone()
    }

    fn tls_config(&self) -> OwnedTLSConfig {
        let mut config = OwnedTLSConfig::default();

        if let (Some(ca), Some(client_cert), Some(client_key)) = (
            self.ca.clone(),
            self.client_cert.clone(),
            self.client_key.clone(),
        ) {
            info!("🔐 AMQP SSL/TLS enabled");
            config.cert_chain = Some(ca.to_string());
            config.identity = Some(OwnedIdentity::PKCS8 {
                pem: client_cert.clone().into_bytes(),
                key: client_key.clone().into_bytes(),
            });
        }

        config
    }
}

impl JwtConfig for Config {
    fn jwt_secret(&self) -> &str {
        &self.jwt_secret_key
    }

    fn access_token_expire_in_minute(&self) -> i64 {
        self.access_token_expire_in_minute
    }

    fn refresh_token_expire_in_days(&self) -> i64 {
        self.refresh_token_expire_in_days
    }

    fn email_verification_token_expire_in_hours(&self) -> i64 {
        self.email_verification_token_expire_in_hours
    }
}
