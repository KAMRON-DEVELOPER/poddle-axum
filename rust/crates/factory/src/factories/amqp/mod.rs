pub mod error;
pub mod implementation;

use std::sync::Arc;

use lapin::Connection;

use crate::factories::tls::TlsConfig;

pub struct AmqpPropagator;

pub trait AmqpConfig {
    type Tls: TlsConfig;

    fn uri(&self) -> String;
    fn tls_config(&self) -> Self::Tls;
}

#[derive(Clone)]
pub struct Amqp {
    connection: Arc<Connection>,
}
