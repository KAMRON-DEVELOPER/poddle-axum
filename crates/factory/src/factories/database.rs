use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tracing::info;

use crate::factories::tls::TlsConfig;

pub trait DatabaseConfig {
    type Tls: TlsConfig;

    fn database_url(&self) -> String;
    fn max_connections(&self) -> u32 {
        100
    }
    fn pg_ssl_mode(&self) -> PgSslMode;
    // fn tls_config(&self) -> Box<dyn TlsConfig>;
    fn tls_config(&self) -> Self::Tls;
}

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn new<T: DatabaseConfig>(cfg: &T) -> Self {
        let mut options: PgConnectOptions =
            cfg.database_url().parse().expect("Invalid database URL");

        options = options.ssl_mode(cfg.pg_ssl_mode());

        let tls_config = cfg.tls_config();

        // if let Some(ca_path) = self.ca_path {
        //     if ca_path.exists() {
        //         options = options.ssl_root_cert(ca_path);
        //     }
        // }
        if let Some(ca) = tls_config.ca() {
            options = options.ssl_root_cert_from_pem(ca.as_bytes().to_owned());
        }

        // if let Some(client_cert_path) = self.client_cert_path {
        //     if client_cert_path.exists() {
        //         options = options.ssl_client_cert(client_cert_path);
        //     }
        // }
        if let Some(client_cert) = tls_config.client_cert() {
            options = options.ssl_client_cert_from_pem(client_cert.as_bytes());
        }

        // if let Some(client_key_path) = self.client_key_path {
        //     if client_key_path.exists() {
        //         options = options.ssl_client_key(client_key_path);
        //     }
        // }
        if let Some(client_key) = tls_config.client_key() {
            options = options.ssl_client_key_from_pem(client_key.as_bytes());
        }

        let pool = PgPoolOptions::new()
            .max_connections(100)
            .connect_with(options)
            .await.unwrap_or_else(|e| {panic!("Couldn't connect to database, {}", e)});

        info!("✅ Postgres pool created.");

        Self { pool }
    }
}
