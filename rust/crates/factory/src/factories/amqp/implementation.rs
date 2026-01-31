use crate::factories::amqp::error::AmqpError;
use crate::factories::amqp::{Amqp, AmqpConfig, AmqpPropagator};
use axum::{Json, http::StatusCode, response::IntoResponse, response::Response};
use lapin::ExchangeKind;
use lapin::options::{
    BasicQosOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use lapin::tcp::OwnedIdentity;
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, options::BasicPublishOptions,
    tcp::OwnedTLSConfig,
};
use opentelemetry::Context;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tracing::{Span, error, info};

use lapin::types::{AMQPValue, FieldTable, ShortString};
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::collections::HashMap;
use tracing_opentelemetry::OpenTelemetrySpanExt;

impl Amqp {
    pub async fn new(cfg: &AmqpConfig) -> Self {
        let uri = cfg.uri.clone();

        let mut config = OwnedTLSConfig::default();

        if let Some(tls_config) = &cfg.tls_config {
            if let (Some(ca), Some(client_cert), Some(client_key)) = (
                tls_config.ca.clone(),
                tls_config.client_cert.clone(),
                tls_config.client_key.clone(),
            ) {
                info!("🔐 AMQP SSL/TLS enabled");
                config.cert_chain = Some(ca.to_string());
                config.identity = Some(OwnedIdentity::PKCS8 {
                    pem: client_cert.clone().into_bytes(),
                    key: client_key.clone().into_bytes(),
                });
            }
        }

        let options = ConnectionProperties::default();

        let connection = Connection::connect_with_config(&uri, options, config)
            .await
            .expect(format!("Failed to connect to RabbitMQ at {}", uri).as_str());
        info!("✅ RabbitMQ connection created");

        Self {
            connection: Arc::new(connection),
        }
    }

    pub async fn channel(&self) -> Channel {
        let channel = self
            .connection
            .create_channel()
            .await
            .expect("Failed to create channel");

        let exchange = "compute";

        // Declare exchange
        channel
            .exchange_declare(
                exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    auto_delete: false,
                    internal: false,
                    nowait: false,
                    passive: false,
                },
                FieldTable::default(),
            )
            .await
            .expect("Failed to declare exchange");

        // Declare queues
        for queue in &["compute.create", "compute.update", "compute.delete"] {
            let mut args = FieldTable::default();
            args.insert(
                "x-dead-letter-exchange".into(),
                AMQPValue::LongString("compute.dead_letter".into()),
            );
            channel
                .queue_declare(
                    queue,
                    QueueDeclareOptions {
                        durable: true,
                        exclusive: false,
                        auto_delete: false,
                        nowait: false,
                        passive: false,
                    },
                    args,
                )
                .await
                .expect("Failed to declare queue");

            channel
                .queue_bind(
                    queue,
                    exchange,
                    queue,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("Failed to bind queue");
        }

        // Set QoS (prefetch)
        channel
            .basic_qos(10, BasicQosOptions::default())
            .await
            .expect("Failed to set basic QoS");

        channel
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

impl AmqpPropagator {
    // Inject current tracing context into lapin FieldTable
    pub fn inject_context(headers: &mut FieldTable) {
        let propagator = TraceContextPropagator::new();
        let mut injector = HashMap::new();

        // Get current span context from tracing
        let cx = Span::current().context();
        propagator.inject_context(&cx, &mut injector);

        for (key, value) in injector {
            headers.insert(ShortString::from(key), AMQPValue::LongString(value.into()));
        }
    }

    // Extract context from lapin FieldTable and return an OTel Context
    pub fn extract_context(headers: &FieldTable) -> Context {
        let propagator = TraceContextPropagator::new();
        let mut extractor = HashMap::new();

        for (key, value) in headers.inner() {
            if let AMQPValue::LongString(val) = value {
                extractor.insert(key.to_string(), val.to_string());
            }
        }

        propagator.extract(&extractor)
    }
}
