use std::sync::Arc;

use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, options::BasicPublishOptions,
    tcp::OwnedTLSConfig,
};
use serde::Serialize;
use shared::utilities::errors::AppError;
use tracing::info;

pub trait AmqpConfig {
    fn uri(&self) -> String;
    fn tls_config(&self) -> OwnedTLSConfig;
}

#[derive(Clone)]
pub struct Amqp {
    connection: Arc<Connection>,
}

impl Amqp {
    pub async fn new<T: AmqpConfig>(cfg: &T) -> Self {
        let uri = cfg.uri();
        let config = cfg.tls_config();

        let options = ConnectionProperties::default();

        let connection = Connection::connect_with_config(&uri, options, config)
            .await
            .expect(format!("Failed to connect to RabbitMQ at {}", uri).as_str());
        info!("✅ RabbitMQ connection established.");

        Self {
            connection: Arc::new(connection),
        }
    }

    pub async fn channel(&self) -> Channel {
        self.connection
            .create_channel()
            .await
            .expect("Couldn't create channel")
    }

    pub async fn basic_publish<T: Serialize>(
        &self,
        exchange: &str,
        routing_key: &str,
        message: &T,
    ) -> Result<(), AppError> {
        let channel = self.channel().await;

        let payload = serde_json::to_vec(message)?;

        channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions {
                    mandatory: false,
                    immediate: false,
                },
                &payload,
                BasicProperties::default()
                    .with_delivery_mode(2)
                    .with_content_type("application/json".into()),
            )
            .await?
            .await?;

        Ok(())
    }

    pub async fn basic_publish_with_properties<T: Serialize>(
        &self,
        exchange: &str,
        routing_key: &str,
        message: &T,
        properties: BasicProperties,
    ) -> Result<(), AppError> {
        let channel = self.channel().await;

        let payload = serde_json::to_vec(message)?;

        channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions {
                    mandatory: false,
                    immediate: false,
                },
                &payload,
                properties,
            )
            .await?
            .await?;

        Ok(())
    }
}
