pub mod error;
pub mod implementation;

use serde::Deserialize;
use sqlx::PgPool;

use crate::factories::tls::TlsConfig;

#[derive(Deserialize, Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
    pub pg_ssl_mode: Option<String>,
    pub max_connections: Option<u32>,
    pub tls_config: Option<TlsConfig>,
}

#[derive(Clone, Debug)]
pub struct Database {
    pub pool: PgPool,
}
