use std::sync::Arc;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::FutureProducer;
use tracing::info;

use crate::factories::{
    kafka::{Kafka, KafkaConfig, error::KafkaError},
    tls::TlsConfig,
};

impl Kafka {
    pub fn new<T: KafkaConfig>(cfg: &T, group_id: &str) -> Result<Self, KafkaError> {
        let tls_config = cfg.tls_config();

        let mut common = ClientConfig::new();
        common.set("bootstrap.servers", cfg.kafka_bootstrap_servers());

        if let (Some(ca), Some(client_cert), Some(client_key)) = (
            tls_config.ca(),
            tls_config.client_cert(),
            tls_config.client_key(),
        ) {
            info!("🔐 Kafka SSL/TLS enabled");
            common
                .set("security.protocol", "ssl")
                .set("ssl.ca.pem", ca)
                .set("ssl.certificate.pem", client_cert)
                .set("ssl.key.pem", client_key);
        }

        let producer = common
            .clone()
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.ms", "1")
            .create::<FutureProducer>()?;

        let consumer = common
            .clone()
            .set("group.id", group_id)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "5000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create::<StreamConsumer>()?;

        info!("✅ Kafka producer and consumer created.");

        Ok(Self {
            producer,
            consumer: Arc::new(consumer),
        })
    }
}
