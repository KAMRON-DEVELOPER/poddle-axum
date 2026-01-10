use std::sync::Arc;

use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties,
    options::BasicPublishOptions,
    tcp::{OwnedIdentity, OwnedTLSConfig},
};
use serde::Serialize;
use tracing::info;

use shared::utilities::{config::Config, errors::AppError};

use crate::factories::tls::TlsConfig;

pub trait AmqpConfig {
    fn uri(&self) -> String;
    fn tls_config(&self) -> impl TlsConfig;
}

#[derive(Clone)]
pub struct Amqp {
    connection: Arc<Connection>,
}

impl Amqp {
    pub async fn new(config: &Config) -> Result<Self, AppError> {
        let uri = config.amqp_addr.clone();

        let options = ConnectionProperties::default();
        let mut tlsconfig = OwnedTLSConfig::default();

        if let (Some(ca), Some(client_cert), Some(client_key)) =
            (&config.ca, &config.client_cert, &config.client_key)
        {
            info!("🔐 AMQP SSL/TLS enabled");
            tlsconfig.cert_chain = Some(ca.to_string());
            tlsconfig.identity = Some(OwnedIdentity::PKCS8 {
                pem: client_cert.clone().into_bytes(),
                key: client_key.clone().into_bytes(),
            });
        }

        let connection = Connection::connect_with_config(&uri, options, tlsconfig).await?;
        info!("✅ RabbitMQ connection established.");

        Ok(Self {
            connection: Arc::new(connection),
        })
    }

    pub async fn channel(&self) -> Result<Channel, AppError> {
        Ok(self.connection.create_channel().await?)
    }

    pub async fn basic_publish<T: Serialize>(
        &self,
        exchange: &str,
        routing_key: &str,
        message: &T,
    ) -> Result<(), AppError> {
        let channel = self.channel().await?;

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
        let channel = self.channel().await?;

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
