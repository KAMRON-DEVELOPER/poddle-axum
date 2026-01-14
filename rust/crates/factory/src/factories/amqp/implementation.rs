use crate::factories::amqp::error::AmqpError;
use crate::factories::amqp::{Amqp, AmqpConfig};
use crate::factories::tls::TlsConfig;
use axum::{Json, http::StatusCode, response::IntoResponse, response::Response};
use lapin::tcp::OwnedIdentity;
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, options::BasicPublishOptions,
    tcp::OwnedTLSConfig,
};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};

impl Amqp {
    pub async fn new<T: AmqpConfig>(cfg: &T) -> Self {
        let uri = cfg.uri();
        let tls_config = cfg.tls_config();

        let mut config = OwnedTLSConfig::default();

        if let (Some(ca), Some(client_cert), Some(client_key)) = (
            tls_config.ca(),
            tls_config.client_cert(),
            tls_config.client_key(),
        ) {
            info!("🔐 AMQP SSL/TLS enabled");
            config.cert_chain = Some(ca.to_string());
            config.identity = Some(OwnedIdentity::PKCS8 {
                pem: client_cert.clone().into_bytes(),
                key: client_key.clone().into_bytes(),
            });
        }

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
            .expect("Failed to create channel")
    }

    pub async fn basic_publish<T: Serialize>(
        &self,
        exchange: &str,
        routing_key: &str,
        message: &T,
    ) -> Result<(), AmqpError> {
        let channel = self.channel().await;

        let payload = serde_json::to_vec(message)?;

        let publisher_confirm = channel
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
            .await;

        match publisher_confirm {
            Ok(publisher_confirm) => match publisher_confirm.await {
                Ok(_) => {}
                Err(e) => {
                    error!(target: "basic_publish_confirmation_error", "basic publish confirmation error: {}", e);
                }
            },
            Err(e) => {
                error!(target: "basic_publish_publisher_confirm_error", "basic publish publisher confirm error: {}", e);
            }
        }

        Ok(())
    }

    pub async fn basic_publish_with_properties<T: Serialize>(
        &self,
        exchange: &str,
        routing_key: &str,
        message: &T,
        properties: BasicProperties,
    ) -> Result<(), AmqpError> {
        let channel = self.channel().await;

        let payload = serde_json::to_vec(message)?;

        let publisher_confirm = channel
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
            .await;

        match publisher_confirm {
            Ok(publisher_confirm) => match publisher_confirm.await {
                Ok(_) => {}
                Err(e) => {
                    error!(target: "basic_publish_with_properties_confirmation_error", "basic publish with properties confirmation error: {}", e);
                }
            },
            Err(e) => {
                error!(target: "basic_publish_with_properties_publisher_confirm_error", "basic publish with properties publisher confirm error: {}", e);
            }
        }

        Ok(())
    }
}

impl IntoResponse for AmqpError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            // Self::LapinError(e) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Self::SerializationError(e) => {
                error!("Failed to serialize in Amqp: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = Json(json!({"error": msg}));

        (status, body).into_response()
    }
}
