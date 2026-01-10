use std::sync::Arc;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::FutureProducer;
use tracing::info;

use shared::utilities::{config::Config, errors::AppError};

#[derive(Clone)]
pub struct Kafka {
    pub producer: FutureProducer,
    pub consumer: Arc<StreamConsumer>,
}

impl Kafka {
    pub fn new(config: &Config, group_id: &str) -> Result<Self, AppError> {
        let mut common = ClientConfig::new();
        common.set("bootstrap.servers", config.kafka_bootstrap_servers.clone());

        if let (Some(ca), Some(cert), Some(key)) =
            (&config.ca, &config.client_cert, &config.client_key)
        {
            info!("🔐 Kafka SSL/TLS enabled");
            common
                .set("security.protocol", "ssl")
                .set("ssl.ca.pem", ca)
                .set("ssl.certificate.pem", cert)
                .set("ssl.key.pem", key);
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
