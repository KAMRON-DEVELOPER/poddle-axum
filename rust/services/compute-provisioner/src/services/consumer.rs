use compute_core::schemas::{
    CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage,
};
use factory::factories::{
    amqp::{Amqp, AmqpPropagator},
    redis::Redis,
};
use futures::StreamExt;
use kube::Client;
use lapin::{
    Consumer,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions,
        BasicRejectOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
};

use sqlx::{Pool, Postgres};
use tokio::task::JoinSet;
use tracing::{Instrument, debug, error, info, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    error::AppError,
    services::{kubernetes_service::KubernetesService, vault_service::VaultService},
};

pub async fn start_consumer(
    amqp: Amqp,
    redis: Redis,
    pool: Pool<Postgres>,
    client: Client,
    domain: String,
    traefik_namespace: String,
    cluster_issuer_name: String,
    ingress_class_name: Option<String>,
    wildcard_certificate_name: String,
    wildcard_certificate_secret_name: String,
    vault_service: VaultService,
) -> Result<(), AppError> {
    let channel = amqp.channel().await;

    let kubernetes_service = KubernetesService {
        client,
        pool,
        redis,
        vault_service,
        domain,
        traefik_namespace,
        cluster_issuer_name,
        ingress_class_name,
        wildcard_certificate_name,
        wildcard_certificate_secret_name,
    };

    kubernetes_service.init().await?;

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
        let mut queue_args = FieldTable::default();
        queue_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString("compute.dead_letter".into()),
        );
        channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                queue_args,
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

pub fn get_retry_count(headers: &FieldTable) -> i64 {
    // x-death is an array of tables
    if let Some(AMQPValue::FieldArray(x_death_array)) = headers.inner().get("x-death") {
        // We look at the first entry (most recent event)
        if let Some(AMQPValue::FieldTable(table)) = x_death_array.as_slice().first() {
            if let Some(AMQPValue::LongLongInt(count)) = table.inner().get("count") {
                return *count;
            }
        }
    }
    0
}

#[tracing::instrument(name = "consumer.handle_create_messages", skip_all)]
async fn handle_create_messages(kubernetes_service: KubernetesService, mut consumer: Consumer) {
    info!("🎯 Create consumer started");

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("Consumer connection error: {}", e);
                continue;
            }
        };

        // Extract Tracing Context
        let headers = delivery
            .properties
            .headers()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let parent_cx = AmqpPropagator::extract_context(&headers);

        // Extract Retry Count
        let retry_count = get_retry_count(&headers);

        // Clone Service for the async block
        let k8s_svc = kubernetes_service.clone();
        let span = info_span!("consumer.handle_create_messages", retry_count = retry_count);
        let _ = span.set_parent(parent_cx);

        // Try to deserialize into structured message
        async move {
            if retry_count > 5 {
                error!("❌ Max retries reached for deployment. Dropping message.");
                // Acking tells RabbitMQ "I'm done with this," even though it failed.
                // This prevents the infinite loop.
                let _ = delivery.ack(BasicAckOptions::default()).await;
                return;
            }

            match serde_json::from_slice::<CreateDeploymentMessage>(&delivery.data) {
                Ok(message) => {
                    debug!("🎯 Create deployment");
                    debug!("    name: {:?}", message.name);
                    debug!("    image: {:?}", message.image);
                    debug!("    port: {:?}", message.port);
                    debug!("    resources: {:?}", message.resources);
                    debug!("    subdomain: {:?}", message.subdomain);
                    debug!("    replicas: {:?}", message.replicas);
                    debug!("    labels: {:?}", message.labels);
                    debug!("    secrets: {:?}", message.secrets);
                    debug!(
                        "    environment_variables: {:?}\n",
                        message.environment_variables
                    );

                    // Now we have ALL the data we need without a database query!
                    match k8s_svc.create(message.clone()).await {
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
                                "❌ Failed to create deployment: {}, (Retry {}), error: {}",
                                message.deployment_id, retry_count, e
                            );

                            // nack(requeue: false) sends it to the DLX
                            if let Err(e) = delivery
                                .nack(BasicNackOptions {
                                    requeue: false,
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
                    if let Err(e) = delivery.reject(BasicRejectOptions { requeue: false }).await {
                        error!("Failed to reject message: {}", e);
                    }
                }
            }
        }
        .instrument(span)
        .await;
    }
}

#[tracing::instrument(name = "consumer.handle_update_messages", skip_all)]
async fn handle_update_messages(kubernetes_service: KubernetesService, mut consumer: Consumer) {
    info!("📏 update consumer started");

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("Consumer connection error: {}", e);
                continue;
            }
        };

        // Extract Tracing Context
        let headers = delivery
            .properties
            .headers()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let parent_cx = AmqpPropagator::extract_context(&headers);

        // Extract Retry Count
        let retry_count = get_retry_count(&headers);

        // Clone Service for the async block
        let k8s_svc = kubernetes_service.clone();
        let span = info_span!("consumer.handle_update_messages", retry_count = retry_count);
        let _ = span.set_parent(parent_cx);

        async move {
            if retry_count > 5 {
                error!("❌ Max retries reached for deployment. Dropping message.");
                // Acking tells RabbitMQ "I'm done with this," even though it failed.
                // This prevents the infinite loop.
                let _ = delivery.ack(BasicAckOptions::default()).await;
                return;
            }

            match serde_json::from_slice::<UpdateDeploymentMessage>(&delivery.data) {
                Ok(message) => {
                    debug!("📏 Updating deployment");
                    debug!("    name: {:?}", message.name);
                    debug!("    image: {:?}", message.image);
                    debug!("    port: {:?}", message.port);
                    debug!("    resources: {:?}", message.resources);
                    debug!("    subdomain: {:?}", message.subdomain);
                    debug!("    replicas: {:?}", message.replicas);
                    debug!("    labels: {:?}", message.labels);
                    debug!("    secrets: {:?}", message.secrets);
                    debug!(
                        "    environment_variables: {:?}\n",
                        message.environment_variables
                    );

                    match k8s_svc.update(message.clone()).await {
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
                            error!(
                                "❌ Failed to update deployment: {}, (Retry {}), error: {}",
                                message.deployment_id, retry_count, e
                            );

                            // nack(requeue: false) sends it to the DLX
                            delivery
                                .nack(BasicNackOptions {
                                    requeue: false,
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
        .instrument(span)
        .await;
    }
}

#[tracing::instrument(name = "consumer.handle_delete_messages", skip_all)]
async fn handle_delete_messages(kubernetes_service: KubernetesService, mut consumer: Consumer) {
    info!("🗑️ Delete consumer started");

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("Consumer connection error: {}", e);
                continue;
            }
        };

        // Extract Tracing Context
        let headers = delivery
            .properties
            .headers()
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let parent_cx = AmqpPropagator::extract_context(&headers);

        // Extract Retry Count
        let retry_count = get_retry_count(&headers);

        // Clone Service for the async block
        let k8s_svc = kubernetes_service.clone();
        let span = info_span!("consumer.handle_delete_messages", retry_count = retry_count);
        let _ = span.set_parent(parent_cx);

        async move {
            if retry_count > 5 {
                error!("❌ Max retries reached for deployment. Dropping message.");
                // Acking tells RabbitMQ "I'm done with this," even though it failed.
                // This prevents the infinite loop.
                let _ = delivery.ack(BasicAckOptions::default()).await;
                return;
            }

            match serde_json::from_slice::<DeleteDeploymentMessage>(&delivery.data) {
                Ok(message) => {
                    info!("🗑️ Deleting deployment {}", message.deployment_id);

                    match k8s_svc.delete(message.clone()).await {
                        Ok(_) => {
                            info!("✅ Deployment {} deleted", message.deployment_id);

                            if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                error!("Failed to ack message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!(
                                "❌ Failed to delete deployment: {}, (Retry {}), error: {}",
                                message.deployment_id, retry_count, e
                            );

                            // nack(requeue: false) sends it to the DLX
                            delivery
                                .nack(BasicNackOptions {
                                    requeue: false,
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
        .instrument(span)
        .await;
    }
}
