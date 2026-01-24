pub mod error;
pub mod implementation;

use std::sync::Arc;

use lapin::Connection;
use serde::Deserialize;

use crate::factories::tls::TlsConfig;

pub struct AmqpPropagator;

#[derive(Deserialize, Debug)]
pub struct AmqpConfig {
    pub uri: String,
    pub tls_config: Option<TlsConfig>,
}

#[derive(Clone)]
pub struct Amqp {
    connection: Arc<Connection>,
}
