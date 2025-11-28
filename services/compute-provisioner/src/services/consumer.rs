use futures::StreamExt;
use lapin::{
    Consumer,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions,
        BasicRejectOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
};
use shared::{
    schemas::{CreateDeploymentMessage, DeleteDeploymentMessage, ScaleDeploymentMessage},
    services::{amqp::Amqp, database::Database, kubernetes::Kubernetes},
    utilities::{config::Config, errors::AppError},
};
use tracing::{error, info, warn};

use crate::services::kubernetes_service::KubernetesService;

pub async fn start_rabbitmq_consumer(
    amqp: Amqp,
    config: Config,
    database: Database,
    kubernetes: Kubernetes,
) -> Result<(), AppError> {
    let channel = amqp.channel().await?;

    // Declare exchange
    channel
        .exchange_declare(
            "compute",
            lapin::ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                auto_delete: false,
                internal: false,
                nowait: false,
                passive: false,
            },
            FieldTable::default(),
        )
        .await?;

    // Declare queues
    for queue_name in &["compute.create", "compute.scale", "compute.delete"] {
        channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,
                    exclusive: false,
                    auto_delete: false,
                    nowait: false,
                    passive: false,
                },
                FieldTable::default(),
            )
            .await?;

        channel
            .queue_bind(
                queue_name,
                "compute",
                queue_name,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;
    }

    // Set QoS (prefetch)
    channel.basic_qos(10, BasicQosOptions::default()).await?;

    // Start consumers
    let create_consumer = channel
        .basic_consume(
            "compute.create",
            "creator",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let scale_consumer = channel
        .basic_consume(
            "compute.scale",
            "scaler",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let delete_consumer = channel
        .basic_consume(
            "compute.delete",
            "deleter",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!("✅ RabbitMQ consumers started");

    // Spawn task for provision consumer
    tokio::spawn(handle_create_messages(
        config.clone(),
        database.clone(),
        kubernetes.clone(),
        create_consumer,
    ));

    // Spawn task for scale consumer
    tokio::spawn(handle_scale_messages(
        database.clone(),
        kubernetes.clone(),
        scale_consumer,
    ));

    // Spawn task for delete consumer
    tokio::spawn(handle_delete_messages(
        database.clone(),
        kubernetes.clone(),
        delete_consumer,
    ));

    Ok(())
}

async fn handle_create_messages(
    config: Config,
    database: Database,
    kubernetes: Kubernetes,
    mut consumer: Consumer,
) {
    info!("🎯 Create consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                // Try to deserialize into structured message
                match serde_json::from_slice::<CreateDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!(
                            "📦 Received create deployment message: {} (image: {}, replicas: {})",
                            message.deployment_id, message.image, message.replicas
                        );

                        // Now we have ALL the data we need without a database query!
                        match KubernetesService::create(
                            &database.pool,
                            &kubernetes.client,
                            message.user_id,
                            message.deployment_id,
                            &config.base_domain,
                            message.clone(),
                        )
                        .await
                        {
                            Ok(_) => {
                                info!(
                                    "✅ Deployment {} created successfully",
                                    message.deployment_id
                                );

                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!("Failed to ack message: {}", e);
                                }
                            }
                            Err(e) => {
                                error!(
                                    "❌ Failed to create deployment {}: {}",
                                    message.deployment_id, e
                                );

                                // Requeue for retry (up to max retries handled by RabbitMQ TTL/DLX)
                                if let Err(e) = delivery
                                    .nack(BasicNackOptions {
                                        requeue: true,
                                        multiple: false,
                                    })
                                    .await
                                {
                                    error!("Failed to nack message: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to parse CreateDeploymentMessage: {}", e);
                        warn!("Payload: {}", String::from_utf8_lossy(&delivery.data));

                        // Don't requeue malformed messages
                        if let Err(e) = delivery.reject(BasicRejectOptions { requeue: false }).await
                        {
                            error!("Failed to reject message: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Consumer error: {}", e);
            }
        }
    }
}

async fn handle_scale_messages(database: Database, kubernetes: Kubernetes, mut consumer: Consumer) {
    info!("📏 Scale consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                match serde_json::from_slice::<ScaleDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!(
                            "📏 Scaling deployment {} to {} replicas",
                            message.deployment_id, message.replicas
                        );

                        match KubernetesService::scale(
                            &database.pool,
                            &kubernetes.client,
                            message.deployment_id,
                            message.user_id,
                            message.replicas,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!(
                                    "✅ Deployment {} scaled to {}",
                                    message.deployment_id, message.replicas
                                );

                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!("Failed to ack message: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to scale deployment: {}", e);

                                delivery
                                    .nack(BasicNackOptions {
                                        requeue: true,
                                        multiple: false,
                                    })
                                    .await
                                    .ok();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse ScaleDeploymentMessage: {}", e);
                        delivery
                            .reject(BasicRejectOptions { requeue: false })
                            .await
                            .ok();
                    }
                }
            }
            Err(e) => error!("Scale consumer error: {}", e),
        }
    }
}

async fn handle_delete_messages(
    database: Database,
    kubernetes: Kubernetes,
    mut consumer: Consumer,
) {
    info!("🗑️ Delete consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                match serde_json::from_slice::<DeleteDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!("🗑️ Deleting deployment {}", message.deployment_id);

                        match KubernetesService::delete(
                            &database.pool,
                            &kubernetes.client,
                            message.deployment_id,
                            message.user_id,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!("✅ Deployment {} deleted", message.deployment_id);

                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!("Failed to ack message: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to delete deployment: {}", e);

                                delivery
                                    .nack(BasicNackOptions {
                                        requeue: true,
                                        multiple: false,
                                    })
                                    .await
                                    .ok();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse DeleteDeploymentMessage: {}", e);
                        delivery
                            .reject(BasicRejectOptions { requeue: false })
                            .await
                            .ok();
                    }
                }
            }
            Err(e) => error!("Delete consumer error: {}", e),
        }
    }
}
