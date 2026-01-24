pub mod error;
pub mod implementation;

use std::sync::Arc;

use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::FutureProducer;
use serde::Deserialize;

use crate::factories::tls::TlsConfig;

#[derive(Deserialize, Clone)]
pub struct KafkaConfig {
    pub bootstrap_servers: String,
    pub tls_config: Option<TlsConfig>,
}

#[derive(Clone)]
pub struct Kafka {
    pub producer: FutureProducer,
    pub consumer: Arc<StreamConsumer>,
}
