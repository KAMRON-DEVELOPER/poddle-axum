use lapin::{
    Consumer,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicQosOptions, ExchangeDeclareOptions,
        QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
};
use shared::{
    services::{amqp::Amqp, database::Database, kubernetes::Kubernetes},
    utilities::{config::Config, errors::AppError},
};
use tracing::info;

use crate::services::kubernetes::DeploymentService;

pub async fn start_rabbitmq_consumer(
    amqp: Amqp,
    database: Database,
    kubernetes: Kubernetes,
    config: Config,
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
    for queue_name in &["compute.provision", "compute.scale", "compute.delete"] {
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
    let provision_consumer = channel
        .basic_consume(
            "compute.provision",
            "provisioner",
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
    tokio::spawn(handle_provision_messages(
        provision_consumer,
        database.clone(),
        kubernetes.clone(),
        config.clone(),
    ));

    // Spawn task for scale consumer
    tokio::spawn(handle_scale_messages(
        scale_consumer,
        database.clone(),
        kubernetes.clone(),
    ));

    // Spawn task for delete consumer
    tokio::spawn(handle_delete_messages(
        delete_consumer,
        database.clone(),
        kubernetes.clone(),
    ));

    Ok(())
}

async fn handle_provision_messages(
    mut consumer: Consumer,
    database: Database,
    kubernetes: Kubernetes,
    config: Config,
) {
    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                let payload = String::from_utf8_lossy(&delivery.data);
                info!("Received provision message: {}", payload);

                match serde_json::from_slice::<serde_json::Value>(&delivery.data) {
                    Ok(message) => {
                        let deployment_id = message["deployment_id"]
                            .as_str()
                            .and_then(|s| uuid::Uuid::parse_str(s).ok());
                        let user_id = message["user_id"]
                            .as_str()
                            .and_then(|s| uuid::Uuid::parse_str(s).ok());

                        if let (Some(deployment_id), Some(user_id)) = (deployment_id, user_id) {
                            // Process deployment creation
                            match DeploymentService::create(
                                &database.pool,
                                &kubernetes.client,
                                user_id,
                                deployment_id,
                                &config.base_domain,
                            )
                            .await
                            {
                                Ok(_) => {
                                    info!("✅ Deployment {} created", deployment_id);
                                    // Acknowledge message
                                    delivery
                                        .ack(BasicAckOptions::default())
                                        .await
                                        .expect("Failed to ack");
                                }
                                Err(e) => {
                                    tracing::error!("❌ Failed to create deployment: {}", e);
                                    // Reject and requeue
                                    delivery
                                        .nack(BasicNackOptions {
                                            requeue: true,
                                            multiple: false,
                                        })
                                        .await
                                        .expect("Failed to nack");
                                }
                            }
                        } else {
                            tracing::error!("Invalid message format");
                            delivery
                                .reject(BasicRejectOptions { requeue: false })
                                .await
                                .expect("Failed to reject");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse message: {}", e);
                        delivery
                            .reject(BasicRejectOptions { requeue: false })
                            .await
                            .expect("Failed to reject");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Consumer error: {}", e);
            }
        }
    }
}

async fn handle_scale_messages(mut consumer: Consumer, database: Database, kubernetes: Kubernetes) {
    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => match serde_json::from_slice::<serde_json::Value>(&delivery.data) {
                Ok(message) => {
                    let deployment_id = message["deployment_id"]
                        .as_str()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());
                    let user_id = message["user_id"]
                        .as_str()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());
                    let replicas = message["replicas"].as_i64();

                    if let (Some(deployment_id), Some(user_id), Some(replicas)) =
                        (deployment_id, user_id, replicas)
                    {
                        match DeploymentService::scale(
                            &database.pool,
                            &kubernetes.client,
                            deployment_id,
                            user_id,
                            replicas as i32,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!("✅ Deployment {} scaled to {}", deployment_id, replicas);
                                delivery.ack(BasicAckOptions::default()).await.ok();
                            }
                            Err(e) => {
                                tracing::error!("Failed to scale: {}", e);
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
                }
                Err(e) => {
                    tracing::error!("Failed to parse scale message: {}", e);
                    delivery
                        .reject(BasicRejectOptions { requeue: false })
                        .await
                        .ok();
                }
            },
            Err(e) => tracing::error!("Scale consumer error: {}", e),
        }
    }
}

async fn handle_delete_messages(
    mut consumer: Consumer,
    database: Database,
    kubernetes: Kubernetes,
) {
    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => match serde_json::from_slice::<serde_json::Value>(&delivery.data) {
                Ok(message) => {
                    let deployment_id = message["deployment_id"]
                        .as_str()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());
                    let user_id = message["user_id"]
                        .as_str()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());

                    if let (Some(deployment_id), Some(user_id)) = (deployment_id, user_id) {
                        match DeploymentService::delete(
                            &database.pool,
                            &kubernetes.client,
                            deployment_id,
                            user_id,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!("✅ Deployment {} deleted", deployment_id);
                                delivery.ack(BasicAckOptions::default()).await.ok();
                            }
                            Err(e) => {
                                tracing::error!("Failed to delete: {}", e);
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
                }
                Err(e) => {
                    tracing::error!("Failed to parse delete message: {}", e);
                    delivery
                        .reject(BasicRejectOptions { requeue: false })
                        .await
                        .ok();
                }
            },
            Err(e) => tracing::error!("Delete consumer error: {}", e),
        }
    }
}
