use futures::StreamExt;
use kube::Client;
use lapin::{
    Consumer,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions,
        BasicRejectOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
};
use shared::{
    schemas::{CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage},
    services::{amqp::Amqp, redis::Redis},
    utilities::errors::AppError,
};
use sqlx::{Pool, Postgres};
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use crate::services::{kubernetes_service::KubernetesService, vault_service::VaultService};

pub async fn start_consumer(
    amqp: Amqp,
    redis: Redis,
    pool: Pool<Postgres>,
    client: Client,
    base_domain: String,
    enable_tls: bool,
    cluster_issuer: String,
    vault_service: VaultService,
) -> Result<(), AppError> {
    let channel = amqp.channel().await?;

    let kubernetes_service = KubernetesService {
        client,
        pool,
        redis,
        vault_service,
        base_domain,
        enable_tls,
        cluster_issuer,
    };

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
    for queue_name in &["compute.create", "compute.update", "compute.delete"] {
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

    let update_consumer = channel
        .basic_consume(
            "compute.update",
            "updater",
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

    // Create a JoinSet to hold our tasks
    let mut set = JoinSet::new();

    set.spawn(handle_create_messages(
        kubernetes_service.clone(),
        create_consumer,
    ));

    set.spawn(handle_update_messages(
        kubernetes_service.clone(),
        update_consumer,
    ));

    set.spawn(handle_delete_messages(
        kubernetes_service.clone(),
        delete_consumer,
    ));

    info!("✅ RabbitMQ consumers started");

    // Wait for ANY task in the set to exit
    // If one of them crashes or finishes, this loop will catch it.
    while let Some(res) = set.join_next().await {
        match res {
            Ok(_) => error!("A consumer task finished unexpectedly!"),
            Err(e) => error!("A consumer task panicked: {}", e),
        }
        // Optional: We can break here to crash the whole service
        // or we could add logic to restart just the specific task.
        break;
    }

    // Clean up the rest
    set.shutdown().await;

    Ok(())
}

async fn handle_create_messages(kubernetes_service: KubernetesService, mut consumer: Consumer) {
    info!("🎯 Create consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                // Try to deserialize into structured message
                match serde_json::from_slice::<CreateDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!("🎯 Create deployment");
                        info!("    name: {:?}", message.name);
                        info!("    image: {:?}", message.image);
                        info!("    port: {:?}", message.port);
                        info!("    resources: {:?}", message.resources);
                        info!("    subdomain: {:?}", message.subdomain);
                        info!("    replicas: {:?}", message.replicas);
                        info!("    labels: {:?}", message.labels);
                        info!("    secrets: {:?}", message.secrets);
                        info!(
                            "    environment_variables: {:?}\n",
                            message.environment_variables
                        );

                        // Now we have ALL the data we need without a database query!
                        match kubernetes_service.create(message.clone()).await {
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

async fn handle_update_messages(kubernetes_service: KubernetesService, mut consumer: Consumer) {
    info!("📏 update consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                match serde_json::from_slice::<UpdateDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!("📏 Updating deployment");
                        info!("    name: {:?}", message.name);
                        info!("    image: {:?}", message.image);
                        info!("    port: {:?}", message.port);
                        info!("    resources: {:?}", message.resources);
                        info!("    subdomain: {:?}", message.subdomain);
                        info!("    replicas: {:?}", message.replicas);
                        info!("    labels: {:?}", message.labels);
                        info!("    secrets: {:?}", message.secrets);
                        info!(
                            "    environment_variables: {:?}\n",
                            message.environment_variables
                        );

                        match kubernetes_service.update(message.clone()).await {
                            Ok(_) => {
                                info!(
                                    "✅ Deployment {:?} updated successfully",
                                    message.deployment_id
                                );

                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!("Failed to ack message: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to update deployment: {}", e);

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
                        error!("Failed to parse updateDeploymentMessage: {}", e);
                        delivery
                            .reject(BasicRejectOptions { requeue: false })
                            .await
                            .ok();
                    }
                }
            }
            Err(e) => error!("update consumer error: {}", e),
        }
    }
}

async fn handle_delete_messages(kubernetes_service: KubernetesService, mut consumer: Consumer) {
    info!("🗑️ Delete consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                match serde_json::from_slice::<DeleteDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!("🗑️ Deleting deployment {}", message.deployment_id);

                        match kubernetes_service.delete(message.clone()).await {
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
