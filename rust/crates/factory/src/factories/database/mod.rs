pub mod error;
pub mod implementation;

use sqlx::{PgPool, postgres::PgSslMode};

use crate::factories::tls::TlsConfig;

pub trait DatabaseConfig {
    type Tls: TlsConfig;

    fn url(&self) -> String;
    fn max_connections(&self) -> u32 {
        100
    }
    fn pg_ssl_mode(&self) -> PgSslMode;
    fn tls_config(&self) -> Self::Tls;
}

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}
