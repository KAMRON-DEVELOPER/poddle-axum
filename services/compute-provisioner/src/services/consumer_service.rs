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
    schemas::{CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage},
    services::{amqp::Amqp, database::Database, kubernetes::Kubernetes},
    utilities::{config::Config, errors::AppError},
};
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use crate::services::{kubernetes_service::KubernetesService, vault_service::VaultService};

pub async fn start_rabbitmq_consumer(
    amqp: Amqp,
    config: Config,
    database: Database,
    kubernetes: Kubernetes,
    _vault_service: VaultService,
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
        config.clone(),
        database.clone(),
        kubernetes.clone(),
        create_consumer,
    ));

    set.spawn(handle_update_messages(
        config.clone(),
        database.clone(),
        kubernetes.clone(),
        update_consumer,
    ));

    set.spawn(handle_delete_messages(
        config.clone(),
        database.clone(),
        kubernetes.clone(),
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

    // Spawn task for provision consumer
    // let create_handle = tokio::spawn(handle_create_messages(
    //     config.clone(),
    //     database.clone(),
    //     kubernetes.clone(),
    //     create_consumer,
    // ));

    // Spawn task for update consumer
    // let update_handle = tokio::spawn(handle_update_messages(
    //     config.clone(),
    //     database.clone(),
    //     kubernetes.clone(),
    //     update_consumer,
    // ));

    // Spawn task for delete consumer
    // let delete_handle = tokio::spawn(handle_delete_messages(
    //     config.clone(),
    //     database.clone(),
    //     kubernetes.clone(),
    //     delete_consumer,
    // ));

    // Wait for the consumers. If any of them crash or finish, this block will resolve.
    // tokio::select! {
    //     _ = create_handle => {
    //         error!("Create consumer task finished unexpectedly");
    //     }
    //     _ = update_handle => {
    //         error!("Update consumer task finished unexpectedly");
    //     }
    //     _ = delete_handle => {
    //         error!("Delete consumer task finished unexpectedly");
    //     }
    // }

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

                        let kubernetes_service = KubernetesService {
                            client: kubernetes.client.clone(),
                            pool: database.pool.clone(),
                            base_domain: config.base_domain.clone(),
                            enable_tls: config.enable_tls,
                            cluster_issuer: config.cluster_issuer.clone(),
                        };

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

async fn handle_update_messages(
    config: Config,
    database: Database,
    kubernetes: Kubernetes,
    mut consumer: Consumer,
) {
    info!("📏 update consumer started");

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                match serde_json::from_slice::<UpdateDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        info!(
                            "📏 Updating deployment {:?} (replicas: {:?})",
                            message.deployment_id, message.replicas
                        );

                        let kubernetes_service = KubernetesService {
                            client: kubernetes.client.clone(),
                            pool: database.pool.clone(),
                            base_domain: config.base_domain.clone(),
                            enable_tls: config.enable_tls,
                            cluster_issuer: config.cluster_issuer.clone(),
                        };

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

async fn handle_delete_messages(
    config: Config,
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

                        let kubernetes_service = KubernetesService {
                            client: kubernetes.client.clone(),
                            pool: database.pool.clone(),
                            base_domain: config.base_domain.clone(),
                            enable_tls: config.enable_tls,
                            cluster_issuer: config.cluster_issuer.clone(),
                        };

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
