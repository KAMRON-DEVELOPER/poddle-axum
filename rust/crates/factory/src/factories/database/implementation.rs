use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use tracing::info;

use crate::factories::database::{Database, DatabaseConfig};

impl Database {
    pub async fn new(cfg: &DatabaseConfig) -> Self {
        let mut options: PgConnectOptions = cfg.url.parse().expect("Invalid database URL");

        let pg_ssl_mode = match cfg.pg_ssl_mode.as_deref() {
            None => PgSslMode::Disable,
            Some("Disable") => PgSslMode::Disable,
            Some("Allow") => PgSslMode::Allow,
            Some("Prefer") => PgSslMode::Prefer,
            Some("Require") => PgSslMode::Require,
            Some("VerifyCa") => PgSslMode::VerifyCa,
            Some("VerifyFull") => PgSslMode::VerifyFull,
            _ => PgSslMode::Disable,
        };

        options = options.ssl_mode(pg_ssl_mode);

        if let Some(tls_config) = &cfg.tls_config {
            if let Some(ca_path) = &tls_config.ca_path {
                if ca_path.exists() {
                    options = options.ssl_root_cert(ca_path);
                }
            }
            if let Some(ca) = &tls_config.ca {
                options = options.ssl_root_cert_from_pem(ca.as_bytes().to_owned());
            }

            if let Some(client_cert_path) = &tls_config.client_cert_path {
                if client_cert_path.exists() {
                    options = options.ssl_client_cert(client_cert_path);
                }
            }
            if let Some(client_cert) = &tls_config.client_cert {
                options = options.ssl_client_cert_from_pem(client_cert.as_bytes());
            }

            if let Some(client_key_path) = &tls_config.client_key_path {
                if client_key_path.exists() {
                    options = options.ssl_client_key(client_key_path);
                }
            }
            if let Some(client_key) = &tls_config.client_key {
                options = options.ssl_client_key_from_pem(client_key.as_bytes());
            }
        }

        let pool = PgPoolOptions::new()
            .max_connections(100)
            .connect_with(options)
            .await
            .unwrap_or_else(|e| panic!("Failed connect to database, {}", e));

        info!("✅ Postgres pool created");

        Self { pool }
    }
}
