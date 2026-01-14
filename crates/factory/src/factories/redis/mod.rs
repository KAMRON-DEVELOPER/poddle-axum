pub mod error;
pub mod implementation;

use redis::{Client, aio::MultiplexedConnection};

use crate::factories::tls::TlsConfig;

pub trait RedisConfig {
    type Tls: TlsConfig;
    fn url(&self) -> Option<String>;
    fn params(&self) -> RedisParams;
    fn tls_config(&self) -> Self::Tls;
}

pub struct RedisParams {
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Clone)]
pub struct Redis {
    pub client: Client,
    pub connection: MultiplexedConnection,
}
