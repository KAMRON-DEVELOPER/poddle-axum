pub mod error;
pub mod implementation;

use redis::{Client, aio::MultiplexedConnection};
use serde::Deserialize;

use crate::factories::tls::TlsConfig;

#[derive(Deserialize, Clone, Debug)]
pub struct RedisParams {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RedisConfig {
    pub url: Option<String>,
    pub params: Option<RedisParams>,
    pub tls_config: Option<TlsConfig>,
}

#[derive(Clone)]
pub struct Redis {
    pub client: Client,
    pub connection: MultiplexedConnection,
}
