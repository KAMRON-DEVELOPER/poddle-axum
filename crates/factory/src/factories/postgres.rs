use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tracing::info;

use shared::utilities::errors::AppError;

pub trait PostgresConfig {
    fn pg_connect_options(&self) -> PgConnectOptions;
    fn max_connections(&self) -> u32 {
        100
    }
}

#[derive(Clone)]
pub struct Postgres {
    pub pool: PgPool,
}

impl Postgres {
    pub async fn new<T: PostgresConfig>(cfg: &T) -> Result<Self, AppError> {
        let options = cfg.pg_connect_options();

        let pool = PgPoolOptions::new()
            .max_connections(100)
            .connect_with(options)
            .await?;

        info!("✅ Postgres pool created.");

        Ok(Self { pool })
    }
}
