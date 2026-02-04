use compute_core::schemas::{
    CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage,
};
use factory::factories::{
    amqp::{Amqp, AmqpPropagator},
    database::Database,
    redis::Redis,
};
use futures::StreamExt;
use lapin::{
    Consumer,
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicRejectOptions},
    types::{AMQPValue, FieldTable},
};

use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tokio::task::JoinSet;
use tracing::{Instrument, debug, error, info, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{error::AppError, services::kubernetes_service::KubernetesService};

#[derive(Clone)]
pub struct ConsumerContext {
    pub database: Database,
    pub redis: Redis,
    pub amqp: Amqp,
    pub k8s: KubernetesService,
}

pub async fn start_consumer(ctx: ConsumerContext) -> Result<(), AppError> {
    let channel = ctx.amqp.channel().await;

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
        ctx.database.pool.clone(),
        ctx.redis.con.clone(),
        ctx.k8s.clone(),
        create_consumer,
    ));
    set.spawn(handle_update_messages(
        ctx.database.pool.clone(),
        ctx.redis.con.clone(),
        ctx.k8s.clone(),
        update_consumer,
    ));
    set.spawn(handle_delete_messages(ctx.k8s.clone(), delete_consumer));

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
async fn handle_create_messages(
    pool: PgPool,
    con: MultiplexedConnection,
    k8s: KubernetesService,
    mut consumer: Consumer,
) {
    info!("🎯 Create consumer started");

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("❌ Consumer connection error: {}", e);
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
        let pool = pool.clone();
        let con = con.clone();
        let k8s = k8s.clone();
        let span = info_span!("consumer.handle_create_messages", retry_count = retry_count);
        let _ = span.set_parent(parent_cx);

        tokio::spawn(
            async move {
                if retry_count > 3 {
                    error!("❌ Max retries reached for create deployment. Dropping message."); 
                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                        error!("❌ Failed to ack for create deployment for max retries: {}", e);
                    }
                    return;
                }

                match serde_json::from_slice::<CreateDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        debug!(deployment_id = %message.deployment_id, "🎯 Create deployment request received");

                        match k8s.create(pool, con, message.clone() ).await {
                            Ok(_) => {
                                info!(deployment_id = %message.deployment_id, "✅ Deployment created");
                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!(deployment_id = %message.deployment_id, "❌ Failed to ack for create message: {}", e);
                                }
                            }
                            Err(e) => {
                                error!(deployment_id = %message.deployment_id,
                                    "❌ Failed to create deployment: {}", e
                                );

                                if let Err(e) = delivery.nack(BasicNackOptions {requeue: false, multiple: false}).await {
                                    error!(deployment_id = %message.deployment_id, "❌ Failed to nack for create deployment: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to parse CreateDeploymentMessage: {}", e);
                        if let Err(e) = delivery.reject(BasicRejectOptions { requeue: false }).await {
                            error!("❌ Failed to reject for create deployment: {}", e);
                        }
                    }
                }
            }
            .instrument(span),
        );
    }
}

#[tracing::instrument(name = "consumer.handle_update_messages", skip_all)]
async fn handle_update_messages(
    pool: PgPool,
    con: MultiplexedConnection,
    k8s: KubernetesService,
    mut consumer: Consumer,
) {
    info!("📏 update consumer started");

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("❌ Consumer connection error: {}", e);
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
        let pool = pool.clone();
        let con = con.clone();
        let k8s = k8s.clone();
        let span = info_span!("consumer.handle_update_messages", retry_count = retry_count);
        let _ = span.set_parent(parent_cx);

        tokio::spawn(
            async move {
                if retry_count > 3 {
                    error!("❌ Max retries reached for update deployment. Dropping message."); 
                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                        error!("❌ Failed to ack for update deployment for max retries: {}", e);
                    }
                    return;
                }

                match serde_json::from_slice::<UpdateDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        debug!(deployment_id = %message.deployment_id, "📏 Update deployment request received");

                        match k8s.update(pool, con, message.clone()).await {
                            Ok(_) => {
                                info!(deployment_id = %message.deployment_id, "📏 Deployment updated");
                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!(deployment_id = %message.deployment_id, "❌ Failed to ack for update deployment: {}", e);
                                }
                            }
                            Err(e) => {
                                error!(deployment_id = %message.deployment_id, "❌ Failed to update deployment: {}", e);

                                if let Err(e) = delivery.nack(BasicNackOptions {requeue: false, multiple: false}).await {
                                    error!(deployment_id = %message.deployment_id, "❌ Failed to nack for update deployment: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to parse updateDeploymentMessage: {}", e);
                        if let Err(e) = delivery.reject(BasicRejectOptions { requeue: false }).await {
                            error!( "❌ Failed to reject for update deployment: {}", e);
                        }
                    }
                }
            }
            .instrument(span),
        );
    }
}

#[tracing::instrument(name = "consumer.handle_delete_messages", skip_all)]
async fn handle_delete_messages(k8s: KubernetesService, mut consumer: Consumer) {
    info!("🗑️ Delete consumer started");

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                error!("❌ Consumer connection error: {}", e);
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
        let k8s = k8s.clone();
        let span = info_span!("consumer.handle_delete_messages", retry_count = retry_count);
        let _ = span.set_parent(parent_cx);

        tokio::spawn(
            async move {
                if retry_count > 3 {
                    error!("❌ Max retries reached for delete deployment. Dropping message.");  
                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                        error!("❌ Failed to ack for delete deployment for max retries: {}", e);
                    }
                    return;
                }

                match serde_json::from_slice::<DeleteDeploymentMessage>(&delivery.data) {
                    Ok(message) => {
                        debug!(deployment_id = %message.deployment_id, "🗑️ Delete deployment request received");

                        match k8s.delete(  message.clone()).await {
                            Ok(_) => {
                                info!(deployment_id = %message.deployment_id, "🗑️ Deployment created");
                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!(deployment_id = %message.deployment_id, "❌ Failed to ack for delete deployment: {}", e);
                                }
                            }
                            Err(e) => {
                                error!(deployment_id = %message.deployment_id, "❌ Failed to delete deployment: {}", e);

                                if let Err(e) = delivery.nack(BasicNackOptions {requeue: false, multiple: false}).await {
                                    error!(deployment_id = %message.deployment_id, "❌ Failed to nack for delete deployment: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to parse DeleteDeploymentMessage: {}", e);
                        if let Err(e) = delivery.reject(BasicRejectOptions { requeue: false }).await {
                            error!("❌ Failed to reject for delete deployment: {}", e);
                        }
                    }
                }
            }
            .instrument(span),
        );
    }
}
