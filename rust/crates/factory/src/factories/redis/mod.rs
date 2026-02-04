pub mod error;
pub mod implementation;

use redis::{Client, aio::MultiplexedConnection};
use serde::Deserialize;
use serde_with::{NoneAsEmptyString, serde_as};

use crate::factories::tls::TlsConfig;

#[serde_as]
#[derive(Deserialize, Clone, Debug)]
pub struct RedisParams {
    #[serde_as(as = "NoneAsEmptyString")]
    pub host: Option<String>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub port: Option<u16>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub username: Option<String>,
    #[serde_as(as = "NoneAsEmptyString")]
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
    pub con: MultiplexedConnection,
}
