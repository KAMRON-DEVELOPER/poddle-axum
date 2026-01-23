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

    // KUBERNETES
    pub domain: String,
    pub traefik_namespace: String,
    pub cluster_issuer_name: String,
    pub ingress_class_name: Option<String>,
    pub wildcard_certificate_name: String,
    pub wildcard_certificate_secret_name: String,

    pub k8s_in_cluster: bool,
    pub k8s_config_path: Option<String>,
    pub k8s_sa_token: String,

    pub label_selector: String,
    pub scrape_interval_seconds: u64,
    pub metric_snapshots_to_keep: u64,
    pub cache_ttl_seconds: u64,

    pub vault_address: String,
    pub vault_auth_mount: String,
    pub vault_auth_role: String,
    pub vault_kv_mount: String,
    pub vault_skip_tls_verify: bool,
    pub vault_connection: String,
    pub vault_auth: String,

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

        let k8s_config_path =
            get_optional_config_value("K8S_KUBECONFIG", Some("K8S_KUBECONFIG"), None).await;
        let k8s_in_cluster =
            get_config_value("K8S_IN_CLUSTER", Some("K8S_IN_CLUSTER"), None, Some(false)).await;
        let k8s_sa_token = get_config_value(
            "K8S_SA_TOKEN",
            Some("K8S_SA_TOKEN"),
            Some(&PathBuf::from(
                "/var/run/secrets/kubernetes.io/serviceaccount/token",
            )),
            None,
        )
        .await;

        let label_selector = get_config_value(
            "LABEL_SELECTOR",
            Some("LABEL_SELECTOR"),
            None,
            Some("managed-by=poddle".to_string()),
        )
        .await;
        let scrape_interval_seconds = get_config_value(
            "SCRAPE_INTERVAL_SECONDS",
            Some("SCRAPE_INTERVAL_SECONDS"),
            None,
            Some(15),
        )
        .await;
        let metric_snapshots_to_keep = get_config_value(
            "METRIC_SNAPSHOTS_TO_KEEP",
            Some("METRIC_SNAPSHOTS_TO_KEEP"),
            None,
            Some(240),
        )
        .await;
        let cache_ttl_seconds =
            get_config_value("CACHE_TTL_SECS", Some("CACHE_TTL_SECONDS"), None, Some(90)).await;

        let vault_address = get_config_value(
            "VAULT_ADDR",
            Some("VAULT_ADDR"),
            None,
            Some("http://vault.poddle.uz:8200".to_string()),
        )
        .await;
        let vault_auth_mount = get_config_value(
            "VAULT_AUTH_MOUNT",
            Some("VAULT_AUTH_MOUNT"),
            None,
            Some("kubernetes".to_string()),
        )
        .await;
        let vault_auth_role = get_config_value(
            "VAULT_AUTH_ROLE",
            Some("VAULT_AUTH_ROLE"),
            None,
            Some("vso".to_string()),
        )
        .await;
        let vault_kv_mount = get_config_value(
            "VAULT_KV_MOUNT",
            Some("VAULT_KV_MOUNT"),
            None,
            Some("kvv2".to_string()),
        )
        .await;
        let vault_skip_tls_verify = get_config_value(
            "VAULT_SKIP_TLS_VERIFY",
            Some("VAULT_SKIP_TLS_VERIFY"),
            None,
            Some(true),
        )
        .await;
        let vault_connection = get_config_value(
            "VAULT_CONNECTION",
            Some("VAULT_CONNECTION"),
            None,
            Some("vault-connection".to_string()),
        )
        .await;
        let vault_auth = get_config_value(
            "VAULT_AUTH",
            Some("VAULT_AUTH"),
            None,
            Some("vault-auth".to_string()),
        )
        .await;

        let domain = get_config_value(
            "DOMAIN",
            Some("DOMAIN"),
            None,
            Some("poddle.uz".to_string()),
        )
        .await;
        let traefik_namespace =
            get_config_value("TRAEFIK_NAMESPACE", Some("TRAEFIK_NAMESPACE"), None, None).await;
        let cluster_issuer_name = get_config_value(
            "CLUSTER_ISSUER_NAME",
            Some("CLUSTER_ISSUER_NAME"),
            None,
            None,
        )
        .await;
        let ingress_class_name =
            get_optional_config_value("INGRESS_CLASS_NAME", Some("INGRESS_CLASS_NAME"), None).await;
        let wildcard_certificate_name = get_config_value(
            "WILDCARD_CERTIFICATE_NAME",
            Some("WILDCARD_CERTIFICATE_NAME"),
            None,
            None,
        )
        .await;
        let wildcard_certificate_secret_name = get_config_value(
            "WILDCARD_CERTIFICATE_SECRET_NAME",
            Some("WILDCARD_CERTIFICATE_SECRET_NAME"),
            None,
            None,
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
            tracing_level,
            k8s_in_cluster,
            k8s_config_path,
            k8s_sa_token,
            label_selector,
            scrape_interval_seconds,
            metric_snapshots_to_keep,
            cache_ttl_seconds,
            vault_address,
            vault_auth_mount,
            vault_auth_role,
            vault_kv_mount,
            vault_skip_tls_verify,
            vault_connection,
            vault_auth,
            domain,
            traefik_namespace,
            cluster_issuer_name,
            ingress_class_name,
            wildcard_certificate_name,
            wildcard_certificate_secret_name,
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
