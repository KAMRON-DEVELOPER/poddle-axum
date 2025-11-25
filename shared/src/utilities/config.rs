#![allow(unused)]
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use sqlx::postgres::PgSslMode;
use tokio::fs;
use tracing::{Level, warn};

use crate::utilities::errors::AppError;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_addres: String,
    pub frontend_endpoint: String,

    pub base_domain: String,

    // KUBERNETES
    pub k8s_in_cluster: bool,
    pub k8s_config_path: Option<String>,
    pub k8s_encryption_key: String,

    pub base_dir: PathBuf,
    pub tracing_level: Level,

    // DATABASE
    pub pg_ssl_mode: PgSslMode,
    pub database_url: String,

    // REDIS
    pub redis_url: String,
    pub redis_host: String,
    pub redis_port: u16,
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
}

impl Config {
    pub async fn init() -> Result<Self, AppError> {
        let k8s_encryption_key = std::env::var("K8S_ENCRYPTION_KEY")
            .expect("K8S_ENCRYPTION_KEY must be set - generate with: openssl rand -base64 32");

        let k8s_config_path =
            get_optional_config_value("K8S_KUBECONFIG", Some("K8S_KUBECONFIG"), None).await?;
        let k8s_in_cluster =
            get_config_value("K8S_IN_CLUSTER", Some("K8S_IN_CLUSTER"), None, Some(false)).await?;

        let base_domain = std::env::var("BASE_DOMAIN").unwrap_or_else(|_| "poddle.uz".to_string());

        let server_addres = get_config_value(
            "SERVER_ADDRES",
            Some("SERVER_ADDRES"),
            None,
            Some("0.0.0.0:8001".to_string()),
        )
        .await?;

        let frontend_endpoint = get_config_value(
            "FRONTEND_ENDPOINT",
            Some("FRONTEND_ENDPOINT"),
            None,
            Some("http://localhost:5173".to_string()),
        )
        .await?;

        let base_dir = find_project_root().unwrap_or_else(|| PathBuf::from("."));

        let debug = get_config_value("DEBUG", Some("DEBUG"), None, Some(false)).await?;

        let tracing_level = get_config_value(
            "TRACING_LEVEL",
            Some("TRACING_LEVEL"),
            None,
            Some(Level::DEBUG),
        )
        .await?;

        let database_url = get_config_value(
            "DATABASE_URL",
            Some("DATABASE_URL"),
            None,
            Some("postgresql://postgres:password@localhost:5432/pinespot_db".to_string()),
        )
        .await?;

        let redis_url = get_config_value(
            "REDIS_URL",
            Some("REDIS_URL"),
            None,
            Some("redis://localhost:6379/0".to_string()),
        )
        .await?;
        let redis_host = get_config_value(
            "REDIS_HOST",
            Some("REDIS_HOST"),
            None,
            Some("localhost".to_string()),
        )
        .await?;
        let redis_port = get_config_value("REDIS_PORT", None, None, Some(6379)).await?;
        let redis_username =
            get_optional_config_value("REDIS_USERNAME", Some("REDIS_USERNAME"), None).await?;
        let redis_password =
            get_optional_config_value("REDIS_PASSWORD", Some("REDIS_PASSWORD"), None).await?;

        let amqp_addr = get_config_value(
            "AMQP_ADDR",
            Some("AMQP_ADDR"),
            None,
            Some("amqp://localhost:5672".to_string()),
        )
        .await?;

        let kafka_bootstrap_servers = get_config_value(
            "KAFKA_BOOTSTRAP_SERVERS",
            Some("KAFKA_BOOTSTRAP_SERVERS"),
            None,
            Some("localhost:9092".to_string()),
        )
        .await?;

        let gcs_bucket_name =
            get_optional_config_value("GCS_BUCKET_NAME", Some("GCS_BUCKET_NAME"), None).await?;
        let gcp_service_account_path = base_dir.join("certs/service-account.json");
        let gcp_service_account = get_optional_config_value(
            "service_account.json",
            Some("SERVICE_ACCOUNT"),
            Some(&gcp_service_account_path),
        )
        .await?;

        let google_oauth_client_id = get_config_value(
            "GOOGLE_OAUTH_CLIENT_ID",
            Some("GOOGLE_OAUTH_CLIENT_ID"),
            None,
            None,
        )
        .await?;
        let google_oauth_client_secret = get_config_value(
            "GOOGLE_OAUTH_CLIENT_SECRET",
            Some("GOOGLE_OAUTH_CLIENT_SECRET"),
            None,
            None,
        )
        .await?;
        let google_oauth_redirect_url = get_config_value(
            "GOOGLE_OAUTH_REDIRECT_URL",
            Some("GOOGLE_OAUTH_REDIRECT_URL"),
            None,
            None,
        )
        .await?;

        let github_oauth_client_id = get_config_value(
            "GITHUB_OAUTH_CLIENT_ID",
            Some("GITHUB_OAUTH_CLIENT_ID"),
            None,
            None,
        )
        .await?;
        let github_oauth_client_secret = get_config_value(
            "GITHUB_OAUTH_CLIENT_SECRET",
            Some("GITHUB_OAUTH_CLIENT_SECRET"),
            None,
            None,
        )
        .await?;
        let github_oauth_redirect_url = get_config_value(
            "GITHUB_OAUTH_REDIRECT_URL",
            Some("GITHUB_OAUTH_REDIRECT_URL"),
            None,
            None,
        )
        .await?;

        let cookie_key = get_config_value("COOKIE_KEY", Some("COOKIE_KEY"), None, None).await?;
        let cookie_secure =
            get_config_value("COOKIE_SECURE", Some("COOKIE_SECURE"), None, None).await?;

        let s3_access_key_id =
            get_optional_config_value("S3_ACCESS_KEY_ID", Some("S3_ACCESS_KEY_ID"), None).await?;
        let s3_secret_key =
            get_optional_config_value("S3_SECRET_KEY", Some("S3_SECRET_KEY"), None).await?;
        let s3_endpoint =
            get_optional_config_value("S3_ENDPOINT", Some("S3_ENDPOINT"), None).await?;
        let s3_region = get_optional_config_value("S3_REGION", Some("S3_REGION"), None).await?;
        let s3_bucket_name =
            get_optional_config_value("S3_BUCKET_NAME", Some("S3_BUCKET_NAME"), None).await?;
        let jwt_secret_key = get_config_value("SECRET_KEY", Some("SECRET_KEY"), None, None).await?;
        let access_token_expire_in_minute = get_config_value(
            "ACCESS_TOKEN_EXPIRE_IN_MINUTE",
            Some("ACCESS_TOKEN_EXPIRE_IN_MINUTE"),
            None,
            None,
        )
        .await?;
        let refresh_token_expire_in_days = get_config_value(
            "REFRESH_TOKEN_EXPIRE_IN_DAYS",
            Some("REFRESH_TOKEN_EXPIRE_IN_DAYS"),
            None,
            None,
        )
        .await?;
        let email_verification_token_expire_in_hours = get_config_value(
            "EMAIL_VERIFICATION_TOKEN_EXPIRE_IN_HOURS",
            Some("EMAIL_VERIFICATION_TOKEN_EXPIRE_IN_HOURS"),
            None,
            None,
        )
        .await?;
        let refresh_token_renewal_threshold_days = get_config_value(
            "REFRESH_TOKEN_RENEWAL_THRESHOLD_DAYS",
            Some("REFRESH_TOKEN_RENEWAL_THRESHOLD_DAYS"),
            None,
            None,
        )
        .await?;

        let email_service_api_key = get_config_value(
            "EMAIL_SERVICE_API_KEY",
            Some("EMAIL_SERVICE_API_KEY"),
            None,
            None,
        )
        .await?;

        // TLS certs: Docker secrets → fallback path
        let ca_path = base_dir.join("certs/ca/ca.pem");
        let ca = get_optional_config_value("ca.pem", Some("CA"), Some(&ca_path)).await?;
        let client_cert_path = base_dir.join("certs/client/client-cert.pem");
        let client_cert = get_optional_config_value(
            "client-cert.pem",
            Some("CLIENT_CERT"),
            Some(&client_cert_path),
        )
        .await?;
        let client_key_path = base_dir.join("certs/client/client-key.pem");
        let client_key =
            get_optional_config_value("client-key.pem", Some("CLIENT_KEY"), Some(&client_key_path))
                .await?;

        let pg_ssl_mode =
            get_config_value("ssl_mode", Some("SSL_MODE"), None, Some(PgSslMode::Disable)).await?;

        let config = Config {
            k8s_in_cluster,
            k8s_config_path,
            k8s_encryption_key,
            base_domain,
            server_addres,
            frontend_endpoint,
            base_dir,
            tracing_level,
            database_url,
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
        };

        Ok(config)
    }
}

fn find_project_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("Cargo.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub async fn get_optional_config_value<T>(
    secret_name: &str,
    env_name: Option<&str>,
    fallback_path: Option<&PathBuf>,
) -> Result<Option<T>, AppError>
where
    T: FromStr,
{
    // Docker secret
    let docker_secret = Path::new("/run/secrets").join(secret_name);
    if docker_secret.exists() {
        if let Ok(content) = fs::read_to_string(&docker_secret).await {
            if let Ok(parsed) = T::from_str(content.trim()) {
                return Ok(Some(parsed));
            }
        }
    }

    // Env var
    if let Some(env_key) = env_name
        && let Ok(val) = std::env::var(env_key)
    {
        if let Ok(parsed) = T::from_str(val.trim()) {
            return Ok(Some(parsed));
        }
    }

    // Fallback path
    if let Some(path) = fallback_path
        && path.exists()
    {
        if let Ok(content) = fs::read_to_string(path).await {
            if let Ok(parsed) = T::from_str(content.trim()) {
                return Ok(Some(parsed));
            }
        }
    }

    Ok(None)
}

pub async fn get_config_value<T>(
    secret_name: &str,
    env_name: Option<&str>,
    fallback_path: Option<&PathBuf>,
    fallback: Option<T>,
) -> Result<T, AppError>
where
    T: FromStr + Clone,
{
    if let Some(value) =
        get_optional_config_value::<T>(secret_name, env_name, fallback_path).await?
    {
        return Ok(value);
    }

    fallback.ok_or_else(|| {
        AppError::EnvironmentVariableNotSetError(env_name.unwrap_or(secret_name).to_string())
    })
}
