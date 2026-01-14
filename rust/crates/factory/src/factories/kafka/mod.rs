pub mod error;
pub mod implementation;

use std::sync::Arc;

use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::FutureProducer;

use crate::factories::tls::TlsConfig;

pub trait KafkaConfig {
    type Tls: TlsConfig;

    fn kafka_bootstrap_servers(&self) -> String;
    fn tls_config(&self) -> Self::Tls;
}

#[derive(Clone)]
pub struct Kafka {
    pub producer: FutureProducer,
    pub consumer: Arc<StreamConsumer>,
}
