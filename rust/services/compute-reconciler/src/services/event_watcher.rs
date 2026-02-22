use chrono::Utc;
use compute_core::cache_keys::CacheKeys;
use compute_core::channel_names::ChannelNames;
use compute_core::crds::{Build, Image};
use compute_core::determiners::determine_deployment_status;
use compute_core::event::{ComputeEvent, EventLevel};
use compute_core::models::DeploymentStatus;
use compute_core::schemas::{
    DeploymentSourceMessage, MetricSnapshot, Pod, PodMeta, PodPhase, UpdateDeploymentMessage,
};
use factory::factories::amqp::{Amqp, AmqpPropagator};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::Pod as K8sPod;
use kube::api::DeleteParams;
use kube::runtime::watcher::{Config as WatcherConfig, Event};
use kube::{Api, Client};
use lapin::BasicProperties;
use lapin::options::BasicPublishOptions;
use lapin::types::FieldTable;
use redis::aio::MultiplexedConnection;
use redis::{AsyncTypedCommands, pipe};
use sqlx::PgPool;
use tracing::{Instrument, debug, error, info, info_span, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppError;

pub async fn event_watcher(
    cfg: Config,
    pool: PgPool,
    mut redis: MultiplexedConnection,
    amqp: Amqp,
    client: Client,
) -> Result<(), AppError> {
    let watcher_config = WatcherConfig::default().labels("poddle.io/managed-by=poddle");

    let deployment: Api<K8sDeployment> = Api::all(client.clone());
    let pod: Api<K8sPod> = Api::all(client.clone());
    let buildkit_job: Api<Job> = Api::all(client.clone());
    let kpack_build: Api<Build> = Api::all(client.clone());

    let mut deployment_stream = kube::runtime::watcher(deployment, watcher_config.clone()).boxed();
    let mut pod_stream = kube::runtime::watcher(pod, watcher_config.clone()).boxed();
    let mut buildkit_job_stream =
        kube::runtime::watcher(buildkit_job, watcher_config.clone()).boxed();
    let mut kpack_build_stream = kube::runtime::watcher(kpack_build, watcher_config).boxed();

    info!("🔍 Starting Kubernetes watchers");
    loop {
        tokio::select! {
            Some(event) = deployment_stream.next() => {
                if let Err(e) = handle_deployment_event(event, &pool, &mut redis).await {
                    error!(error = %e, "❌ Failed to handle deployment event: {}", e);
                }
            }
            Some(event) = pod_stream.next() => {
                if let Err(e) = handle_pod_event(event, &cfg, &pool, &mut redis).await {
                    error!(error = %e, "❌ Failed to handle pod event");
                }
            }
            Some(event) = buildkit_job_stream.next() => {
                if let Err(e) = handle_buildkit_job_event(event, &cfg, &pool, &mut redis, &amqp).await {
                    error!(error = %e, "❌ Failed to handle job event");
                }
            }
            Some(event) = kpack_build_stream.next() => {
                if let Err(e) = handle_kpack_build_event(event, &pool, &mut redis, &amqp, &client).await {
                    error!(error = %e, "❌ Failed to handle kpack build event");
                }
            }
            else => {
                warn!("❌ Both watcher streams ended unexpectedly");
                break;
            }
        }
    }

    Ok(())
}

#[tracing::instrument("handle_deployment_event", skip_all, err)]
async fn handle_deployment_event(
    event: Result<Event<K8sDeployment>, kube::runtime::watcher::Error>,
    pool: &PgPool,
    con: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    match event {
        Ok(Event::Apply(deployment)) => {
            // Extract deployment info
            let name = deployment.metadata.name.as_ref().unwrap();
            let ns = deployment.metadata.namespace.as_ref().unwrap();
            let labels = deployment.metadata.labels.as_ref();
            let project_id = labels
                .and_then(|l| l.get("poddle.io/project-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let deployment_id = labels
                .and_then(|l| l.get("poddle.io/deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if project_id.is_none() || deployment_id.is_none() {
                // Not our deployment, skip
                return Ok(());
            }

            // If the deployment is deleted in the DB, ignore this Pod event.
            let is_active = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM deployments WHERE id = $1 AND status != 'deleted')",
                deployment_id
            )
            .fetch_one(pool)
            .await?
            .unwrap_or(false);

            if !is_active {
                return Ok(());
            }

            let project_id = project_id.unwrap();
            let deployment_id = deployment_id.unwrap();

            // Extract status information
            let spec = deployment.spec.as_ref().unwrap();
            let status = deployment.status.as_ref();

            let desired = spec.replicas.unwrap_or(1);
            let available = status.and_then(|s| s.available_replicas).unwrap_or(0);
            let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let updated = status.and_then(|s| s.updated_replicas).unwrap_or(0);

            // Determine deployment status based on replica states
            let new_status = determine_deployment_status(desired, ready, available, updated);

            info!(
                project_id = %project_id,
                deployment_id = %deployment_id,
                ns = %ns,
                name = %name,
                ready = %ready,
                desired = %desired,
                new_status = %new_status,
                "📥 Deployment Event::Apply received",
            );

            // Initialize redis deployment metrics structure
            let metrics_key = CacheKeys::deployment_metrics(&deployment_id.to_string());
            let exists = con.exists(&metrics_key).await?;
            if !exists {
                let ts = Utc::now().timestamp();
                let cpu = 0.0;
                let memory = 0.0;
                let idle_snapshot = MetricSnapshot { ts, cpu, memory };
                con.lpush(&metrics_key, idle_snapshot).await?;
            }

            // Update database
            let query_result = sqlx::query!(
                r#"
                UPDATE deployments
                SET status = $2
                WHERE id = $1
                "#,
                deployment_id,
                new_status as DeploymentStatus
            )
            .execute(pool)
            .await?;

            if query_result.rows_affected() == 0 {
                when_affacted_rows_zero(&project_id, &deployment_id, con).await?
            }

            let message = ComputeEvent::DeploymentStatusUpdate {
                id: &deployment_id,
                status: new_status,
            };
            send_deployments_message(&project_id, message, con).await?;
        }
        Ok(Event::Delete(deployment)) => {
            let labels = deployment.metadata.labels.as_ref();
            let project_id = labels
                .and_then(|l| l.get("poddle.io/project-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let deployment_id = labels
                .and_then(|l| l.get("poddle.io/deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if project_id.is_none() || deployment_id.is_none() {
                // Not our deployment, skip
                return Ok(());
            }

            let project_id = project_id.unwrap();
            let deployment_id = deployment_id.unwrap();

            info!(
                project_id = %project_id,
                deployment_id = %deployment_id,
                "📥 Deployment Event::Delete received",
            );

            let dep_id = deployment_id.to_string();
            let index_keys = CacheKeys::deployment_pods(&dep_id);

            // fetch pod uids
            // let uids: Vec<String> = con.zrange(&index_keys, 0, -1).await?;

            let mut p = pipe();

            // delete all pod data
            // for uid in uids {
            //     p.del(CacheKeys::deployment_pod_meta(&dep_id, &uid))
            //         .ignore();
            //     p.del(CacheKeys::deployment_pod_metrics(&dep_id, &uid))
            //         .ignore();
            // }

            // delete deployment metrics & index keys
            p.del(CacheKeys::deployment_metrics(&dep_id)).ignore();
            p.del(index_keys).ignore();

            p.query_async::<()>(con).await?;

            let message = ComputeEvent::DeploymentStatusUpdate {
                id: &deployment_id,
                status: DeploymentStatus::Deleted,
            };
            send_deployments_message(&project_id, message, con).await?
        }
        Ok(Event::Init) => info!("✅ Watcher started init phase"),
        Ok(Event::InitApply(_)) => {}
        Ok(Event::InitDone) => info!("✅ Watcher initialization complete - State synced"),
        Err(e) => error!("⚠️ Watcher error: {}", e),
    }

    Ok(())
}

#[tracing::instrument("handle_pod_event", skip_all, err)]
async fn handle_pod_event(
    event: Result<Event<K8sPod>, kube::runtime::watcher::Error>,
    _cfg: &Config,
    pool: &PgPool,
    con: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    match event {
        Ok(Event::Apply(pod)) => {
            let labels = pod.metadata.labels.as_ref();
            let ns = pod.metadata.namespace;
            let project_id = labels
                .and_then(|l| l.get("poddle.io/project-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let deployment_id = labels
                .and_then(|l| l.get("poddle.io/deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if project_id.is_none() || deployment_id.is_none() {
                // Not our deployment, skip
                return Ok(());
            }

            // If the deployment is deleted in the DB, ignore this Pod event.
            let is_active = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM deployments WHERE id = $1 AND status != 'deleted')",
                deployment_id
            )
            .fetch_one(pool)
            .await?
            .unwrap_or(false);

            if !is_active {
                return Ok(());
            }

            let project_id = project_id.unwrap();
            let deployment_id = deployment_id.unwrap();

            let uid = pod.metadata.uid.as_ref().unwrap().to_string();
            let name = pod.metadata.name.as_ref().unwrap().to_string();
            let phase = pod
                .status
                .as_ref()
                .and_then(|s| s.phase.as_deref())
                .unwrap_or("Unknown")
                .into();
            let mut restart_count = 0;

            // Health Analysis
            let mut crash_reason: Option<String> = None;
            let mut crash_message: Option<String> = None;

            if let Some(statuses) = pod
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
            {
                for status in statuses {
                    restart_count += status.restart_count;

                    if let Some(waiting) = status.state.as_ref().and_then(|s| s.waiting.as_ref()) {
                        let reason = waiting.reason.as_deref().unwrap_or_default();
                        if matches!(
                            reason,
                            "CrashLoopBackOff" | "ImagePullBackOff" | "ErrImagePull"
                        ) {
                            crash_reason = Some(reason.to_string());
                            crash_message = waiting.message.clone();
                        }
                    }
                }
            }

            if let Some(reason) = &crash_reason {
                warn!(
                    deployment_id = %deployment_id,
                    "⚠️ Pod {} in namespace {:?} is unhealthy: {} (restarts: {})",
                    name, ns, reason, restart_count
                );

                let is_image_error = reason == "ImagePullBackOff" || reason == "ErrImagePull";

                if is_image_error {
                    sqlx::query!(
                        r#"
                        UPDATE deployments
                        SET status = 'image_pull_error'
                        WHERE id = $1 AND status != 'image_pull_error'
                        "#,
                        deployment_id
                    )
                    .execute(pool)
                    .await?;

                    // rate limit using Redis SETNX + TTL
                    let notified_key =
                        CacheKeys::deployment_image_error_notified(&deployment_id.to_string());

                    let first_time: bool = con.set_nx(&notified_key, 1).await?;

                    if first_time {
                        // 5 minutes TTL (or longer)
                        con.expire(&notified_key, 300).await?;

                        let mut msg = "Image pull failed. This image may be private or credentials are missing/invalid.".to_string();
                        if let Some(detail) = &crash_message {
                            msg.push_str(&format!(" Details: {}", detail));
                        }

                        let message = ComputeEvent::DeploymentSystemMessage {
                            id: &deployment_id,
                            message: msg,
                            level: EventLevel::Error,
                        };
                        send_deployments_message(&project_id, message, con).await?;
                        let message = ComputeEvent::DeploymentStatusUpdate {
                            id: &deployment_id,
                            status: DeploymentStatus::ImagePullError,
                        };
                        send_deployments_message(&project_id, message, con).await?;
                    }
                } else {
                    sqlx::query!(
                        r#"
                        UPDATE deployments
                        SET status = 'unhealthy'
                        WHERE id = $1 AND status NOT IN ('unhealthy', 'failed', 'suspended')
                        "#,
                        deployment_id
                    )
                    .execute(pool)
                    .await?;

                    // keep your CrashLoopBackOff restart-based spam control if you want
                    if restart_count > 0 && restart_count % 3 == 0 {
                        let message = ComputeEvent::DeploymentSystemMessage {
                            id: &deployment_id,
                            message: format!("Deployment is crashing: {}", reason),
                            level: EventLevel::Error,
                        };
                        send_deployments_message(&project_id, message, con).await?;
                        let message = ComputeEvent::DeploymentStatusUpdate {
                            id: &deployment_id,
                            status: DeploymentStatus::Unhealthy,
                        };
                        send_deployments_message(&project_id, message, con).await?;
                    }
                }
            }

            info!(
                project_id = %project_id,
                deployment_id = %deployment_id,
                uid = %uid,
                name = %name,
                phase = %phase,
                restart_count = %restart_count,
                "📥 Pod Event::Apply received",
            );

            let mut p = pipe();
            let dep_id = &deployment_id.to_string();

            let index_key = CacheKeys::deployment_pods(dep_id);
            let meta_key = CacheKeys::deployment_pod_meta(dep_id, &uid);
            let metrics_key = CacheKeys::deployment_pod_metrics(dep_id, &uid);

            let score = Utc::now().timestamp();
            p.zadd(&index_key, &uid, score).ignore();

            let meta = PodMeta {
                uid,
                name,
                phase,
                restart_count,
            };
            let items = meta.as_redis_items();
            p.hset_multiple(&meta_key, &items).ignore();

            // Initialize redis deployment metrics structure
            let exists = con.exists(&metrics_key).await?;
            if !exists {
                let ts = Utc::now().timestamp();
                let cpu = 0.0;
                let memory = 0.0;
                let idle_snapshot = MetricSnapshot { ts, cpu, memory };
                con.lpush(&metrics_key, idle_snapshot).await?;
            }

            let channel = ChannelNames::deployment_metrics(dep_id);
            let message = ComputeEvent::PodApply {
                pod: Pod {
                    meta,
                    ..Default::default()
                },
            };
            p.publish(channel, message);
            p.query_async::<()>(con).await?;
        }
        Ok(Event::Delete(pod)) => {
            let labels = pod.metadata.labels.as_ref();
            let project_id = labels
                .and_then(|l| l.get("poddle.io/project-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let deployment_id = labels
                .and_then(|l| l.get("poddle.io/deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if project_id.is_none() || deployment_id.is_none() {
                // Not our deployment, skip
                return Ok(());
            }

            let project_id = project_id.unwrap();
            let deployment_id = deployment_id.unwrap();

            let uid = pod.metadata.uid.as_ref().unwrap().to_string();
            let name = pod.metadata.name.as_ref().unwrap().to_string();
            let phase: PodPhase = pod
                .status
                .as_ref()
                .and_then(|s| s.phase.as_deref())
                .unwrap_or("Unknown")
                .into();
            let mut restart_count = 0;

            if let Some(statuses) = pod
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
            {
                for status in statuses {
                    restart_count += status.restart_count;
                }
            }

            info!(
                project_id = %project_id,
                deployment_id = %deployment_id,
                uid = %uid,
                name = %name,
                phase = %phase,
                restart_count = %restart_count,
                "📥 Pod Event::Delete received",
            );

            let mut p = pipe();

            let index_key = CacheKeys::deployment_pods(&deployment_id.to_string());
            let meta_key = CacheKeys::deployment_pod_meta(&deployment_id.to_string(), &uid);
            let metrics_key = CacheKeys::deployment_pod_metrics(&deployment_id.to_string(), &uid);

            p.zrem(index_key, &uid).ignore();
            p.del(&meta_key).ignore();
            p.del(&metrics_key).ignore();

            let channel = ChannelNames::deployment_metrics(&deployment_id.to_string());
            let message = ComputeEvent::PodDelete { uid };
            p.publish(channel, message);
            p.query_async::<()>(con).await?;
        }
        Ok(Event::Init) | Ok(Event::InitApply(_)) | Ok(Event::InitDone) => {}
        Err(e) => error!("❌ Pod watcher error: {}", e),
    }

    Ok(())
}

#[tracing::instrument("handle_buildkit_job_event", skip_all, err)]
async fn handle_buildkit_job_event(
    event: Result<Event<Job>, kube::runtime::watcher::Error>,
    _cfg: &Config,
    pool: &PgPool,
    con: &mut MultiplexedConnection,
    amqp: &Amqp,
) -> Result<(), AppError> {
    match event {
        Ok(Event::Apply(job)) => {
            let labels = job.metadata.labels.as_ref();

            let name = job.metadata.name.as_ref().unwrap();
            let project_id = labels
                .and_then(|l| l.get("poddle.io/project-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let deployment_id = labels
                .and_then(|l| l.get("poddle.io/deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let build_id = labels.and_then(|l| l.get("poddle.io/build-id"));

            if project_id.is_none() || deployment_id.is_none() || build_id.is_none() {
                return Ok(());
            }

            let project_id = project_id.unwrap();
            let deployment_id = deployment_id.unwrap();
            let build_id = build_id.unwrap();

            // Check Status
            let succeeded = job.status.as_ref().and_then(|s| s.succeeded).unwrap_or(0);
            let failed = job.status.as_ref().and_then(|s| s.failed).unwrap_or(0);

            if succeeded > 0 {
                info!("✅ Build Job {} Succeeded", name);

                // Construct the new image string, this depends on build
                let image = format!(
                    "me-central1-docker.pkg.dev/poddle-mvp/buildkit/{}",
                    build_id
                );

                let user_id = sqlx::query_scalar!(
                    "SELECT user_id FROM deployments WHERE id = $1",
                    deployment_id
                )
                .fetch_one(pool)
                .await?;

                // We send an Update message. The worker will receive this, see the `image` field,
                // fetch the rest of the config (env vars, ports) from the DB, and run apply_deployment.
                let message = UpdateDeploymentMessage {
                    user_id,
                    project_id,
                    deployment_id,
                    source: Some(DeploymentSourceMessage::InternalBuildComplete { url: image }),
                    name: None,
                    port: None,
                    desired_replicas: None,
                    preset_id: None,
                    resource_spec: None,
                    secrets: None,
                    environment_variables: None,
                    labels: None,
                    domain: None,
                    subdomain: None,
                    timestamp: Utc::now().timestamp(),
                };

                let channel = amqp.channel().await;

                let payload = serde_json::to_vec(&message)?;

                let mut headers = FieldTable::default();
                AmqpPropagator::inject_context(&mut headers);

                // Publish message
                channel
                    .basic_publish(
                        "compute",
                        "compute.update",
                        BasicPublishOptions::default(),
                        &payload,
                        BasicProperties::default()
                            .with_delivery_mode(2)
                            .with_content_type("application/json".into())
                            .with_headers(headers),
                    )
                    .instrument(info_span!("basic_publish.compute.update"))
                    .await?
                    .await?;

                info!(
                    "📤 Published deployment update message for {}",
                    deployment_id
                );
            } else if failed > 0 {
                error!("❌ Build Job {} Failed", name);

                sqlx::query!(
                    "UPDATE deployments SET status = 'build_failed' WHERE id = $1",
                    deployment_id
                )
                .execute(pool)
                .await?;

                let message = ComputeEvent::DeploymentStatusUpdate {
                    id: &deployment_id,
                    status: DeploymentStatus::BuildFailed,
                };
                let channel = ChannelNames::deployments_metrics(&project_id.to_string());
                con.publish(channel, message).await?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[tracing::instrument("handle_kpack_build_event", skip_all, err)]
async fn handle_kpack_build_event(
    event: Result<Event<Build>, kube::runtime::watcher::Error>,
    pool: &PgPool,
    con: &mut MultiplexedConnection,
    amqp: &Amqp,
    client: &Client,
) -> Result<(), AppError> {
    match event {
        Ok(Event::Apply(build)) => {
            let name = build.metadata.name.clone().unwrap_or_default();
            let labels = build.metadata.labels.as_ref();

            let deployment_id = labels
                .and_then(|l| l.get("poddle.io/deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let project_id = labels
                .and_then(|l| l.get("poddle.io/project-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            let build_id = labels.and_then(|l| l.get("poddle.io/build-id"));

            if project_id.is_none() || deployment_id.is_none() || build_id.is_none() {
                return Ok(());
            }
            let project_id = project_id.unwrap();
            let deployment_id = deployment_id.unwrap();
            let build_id = build_id.unwrap();

            // kpack updates the status block as the build progresses
            if let Some(status) = build.status {
                if let Some(conditions) = status.conditions {
                    for cond in conditions {
                        if cond.r#type == "Succeeded" {
                            if cond.status == "True" {
                                let image = status.latest_image.clone().unwrap_or_default();

                                debug!(
                                    "status.latest_image in handle_kpack_build_event: {}",
                                    image
                                );

                                if image.is_empty() {
                                    error!(
                                        "❌ kpack build {} succeeded but no latest_image found in status",
                                        name
                                    );
                                    // Still clean up the Image so kpack stops watching.
                                    delete_kpack_image(client, &build_id).await;
                                    return Ok(());
                                }

                                // Get user_id for the message
                                let user_id = sqlx::query_scalar!(
                                    "SELECT user_id FROM deployments WHERE id = $1",
                                    deployment_id
                                )
                                .fetch_one(pool)
                                .await?;

                                // 3. Construct the Update message to trigger the final Deployment
                                let message = UpdateDeploymentMessage {
                                    user_id,
                                    project_id,
                                    deployment_id,
                                    source: Some(DeploymentSourceMessage::InternalBuildComplete {
                                        url: image,
                                    }),
                                    name: None,
                                    port: None,
                                    desired_replicas: None,
                                    preset_id: None,
                                    resource_spec: None,
                                    secrets: None,
                                    environment_variables: None,
                                    labels: None,
                                    domain: None,
                                    subdomain: None,
                                    timestamp: Utc::now().timestamp(),
                                };

                                let channel = amqp.channel().await;
                                let payload = serde_json::to_vec(&message)?;
                                let mut headers = FieldTable::default();
                                AmqpPropagator::inject_context(&mut headers);

                                // 4. Publish to Provisioner
                                channel
                                    .basic_publish(
                                        "compute",
                                        "compute.update",
                                        BasicPublishOptions::default(),
                                        &payload,
                                        BasicProperties::default()
                                            .with_delivery_mode(2)
                                            .with_content_type("application/json".into())
                                            .with_headers(headers),
                                    )
                                    .instrument(info_span!("basic_publish.compute.update"))
                                    .await?
                                    .await?;

                                info!(
                                    "📤 Published deployment update message for completed kpack build {}",
                                    deployment_id
                                );

                                // Delete the Image so kpack stops watching the repo.
                                // The GitHub App token in the clone URL is short-lived anyway.
                                delete_kpack_image(client, &build_id).await;
                            } else if cond.status == "False" {
                                error!("❌ kpack build {} failed. Reason: {:?}", name, cond.reason);
                                sqlx::query!(
                                    "UPDATE deployments SET status = 'build_failed' WHERE id = $1",
                                    deployment_id
                                )
                                .execute(pool)
                                .await?;

                                let channel =
                                    ChannelNames::deployment_status(&deployment_id.to_string());
                                let message = ComputeEvent::DeploymentStatusUpdate {
                                    id: &deployment_id,
                                    status: DeploymentStatus::BuildFailed,
                                };
                                con.publish(channel, message).await?;

                                // Delete the Image so kpack stops retrying.
                                delete_kpack_image(client, &build_id).await;
                            }
                        }
                    }
                }
            }
        }
        Ok(Event::Delete(build)) => {
            let name = build.metadata.name.unwrap_or_default();
            info!("🗑️ kpack build {} was deleted", name);
        }
        _ => {}
    }

    Ok(())
}

async fn when_affacted_rows_zero(
    project_id: &Uuid,
    deployment_id: &Uuid,
    con: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    {
        let channel = ChannelNames::deployments_metrics(&project_id.to_string());
        let message = ComputeEvent::DeploymentSystemMessage {
            id: deployment_id,
            level: EventLevel::Error,
            message: "❌ Internal server error".to_string(),
        };
        con.publish(channel, message)
            .instrument(info_span!("pubsub.message"))
            .await?;
        warn!(
            project_id = %project_id,
            deployment_id = %deployment_id,
            "❌ Update deployment status affected zero rows"
        );

        Ok(())
    }
}

async fn send_deployments_message(
    id: &Uuid,
    msg: ComputeEvent<'_>,
    con: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    let channel = ChannelNames::deployments_metrics(&id.to_string());
    con.publish(channel, msg)
        .instrument(info_span!("pubsub.status_update"))
        .await?;

    Ok(())
}

/// Delete the kpack Image resource so kpack stops polling the repo.
/// The clone URL contains a short-lived GitHub App token, so any retry would fail anyway.
/// We silently ignore 404s (already gone) and log other errors without propagating them.
async fn delete_kpack_image(client: &Client, image_name: &str) {
    let api: Api<Image> = Api::namespaced(client.clone(), "kpack-build");
    match api.delete(image_name, &DeleteParams::default()).await {
        Ok(_) => info!(
            "🗑️ Deleted kpack Image '{}' after build finished",
            image_name
        ),
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            debug!(
                "kpack Image '{}' already gone, nothing to delete",
                image_name
            );
        }
        Err(e) => {
            // Non-fatal: the Image will just sit there until manual cleanup.
            // The build result has already been processed correctly.
            error!("⚠️ Failed to delete kpack Image '{}': {}", image_name, e);
        }
    }
}
