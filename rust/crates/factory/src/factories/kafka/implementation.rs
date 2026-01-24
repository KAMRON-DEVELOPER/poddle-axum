use std::sync::Arc;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::FutureProducer;
use tracing::info;

use crate::factories::kafka::{Kafka, KafkaConfig, error::KafkaError};

impl Kafka {
    pub fn new(cfg: &KafkaConfig, group_id: &str) -> Result<Self, KafkaError> {
        let mut common = ClientConfig::new();
        common.set("bootstrap.servers", cfg.bootstrap_servers.clone());

        if let Some(tls_config) = &cfg.tls_config {
            if let (Some(ca), Some(client_cert), Some(client_key)) = (
                tls_config.ca.clone(),
                tls_config.client_cert.clone(),
                tls_config.client_key.clone(),
            ) {
                info!("🔐 Kafka SSL/TLS enabled");
                common
                    .set("security.protocol", "ssl")
                    .set("ssl.ca.pem", ca)
                    .set("ssl.certificate.pem", client_cert)
                    .set("ssl.key.pem", client_key);
            }
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
