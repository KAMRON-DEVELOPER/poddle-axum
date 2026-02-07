use compute_core::cache_keys::CacheKeys;
use compute_core::channel_names::ChannelNames;
use compute_core::determiners::determine_deployment_status;
use compute_core::event::{ComputeEvent, EventLevel};
use compute_core::models::DeploymentStatus;
use compute_core::schemas::{Pod, PodMeta};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use k8s_openapi::api::core::v1::Pod as K8sPod;
use kube::runtime::watcher::{Config as WatcherConfig, Event};
use kube::{Api, Client};
use redis::aio::MultiplexedConnection;
use redis::{AsyncTypedCommands, pipe};
use sqlx::PgPool;
use tracing::{Instrument, error, info, info_span, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::error::AppError;

pub async fn event_watcher(
    pool: PgPool,
    cfg: Config,
    mut redis: MultiplexedConnection,
    client: Client,
) -> Result<(), AppError> {
    let watcher_config = WatcherConfig::default().labels("poddle.io/managed-by=poddle");

    let deployment: Api<K8sDeployment> = Api::all(client.clone());
    let pod: Api<K8sPod> = Api::all(client.clone());

    let mut deployment_stream = kube::runtime::watcher(deployment, watcher_config.clone()).boxed();
    let mut pod_stream = kube::runtime::watcher(pod, watcher_config).boxed();

    info!("🔍 Starting Kubernetes watchers");
    loop {
        tokio::select! {
            Some(event) = deployment_stream.next() => {
                if let Err(e) = handle_deployment_event(event, &pool, &mut redis).await {
                    error!(error = %e, "❌ Failed to handle deployment event: {}", e);
                }
            }
            Some(event) = pod_stream.next() => {
                if let Err(e) = handle_pod_event(event,&cfg, &pool, &mut redis).await {
                    error!(error = %e, "❌ Failed to handle pod event");
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
                "📊 Deployment {} in namespace: {}, stats: {:?} ({}/{} ready)",
                name, ns, new_status, ready, desired
            );

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
            send_deployments_message(&project_id, message, con).await?
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
                "🗑️ Deployment was deleted from cluster",
            );

            let dep_id = deployment_id.to_string();
            let index_keys = CacheKeys::deployment_pods(&dep_id);

            // fetch pod uids
            let uids: Vec<String> = con.zrange(&index_keys, 0, -1).await?;

            // delete all pod data
            let mut p = pipe();
            for uid in uids {
                p.del(CacheKeys::deployment_pod_meta(&dep_id, &uid))
                    .ignore();
                p.del(CacheKeys::deployment_pod_metrics(&dep_id, &uid))
                    .ignore();
            }

            // delete deployment keys
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

#[tracing::instrument("handle_deployment_event", skip_all, err)]
async fn handle_pod_event(
    event: Result<Event<K8sPod>, kube::runtime::watcher::Error>,
    cfg: &Config,
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

            if let Some(statuses) = pod
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
            {
                for status in statuses {
                    restart_count += status.restart_count;

                    if let Some(waiting) = &status.state.as_ref().and_then(|s| s.waiting.as_ref()) {
                        let reason = waiting.reason.as_deref().unwrap_or_default();
                        if reason == "CrashLoopBackOff"
                            || reason == "ImagePullBackOff"
                            || reason == "ErrImagePull"
                        {
                            crash_reason = Some(reason.to_string());
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

                // Update DB Status (Only if strictly needed)
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

                if restart_count > 0 && restart_count % 3 == 0 {
                    let message = ComputeEvent::DeploymentSystemMessage {
                        id: &deployment_id,
                        message: format!("Deployment is crashing: {}", reason),
                        level: EventLevel::Error,
                    };
                    send_deployments_message(&project_id, message, con).await?
                }
            }

            let mut p = pipe();
            let ttl = cfg.prometheus.scrape_interval_secs * cfg.prometheus.snapshots_to_keep;
            let meta_key = CacheKeys::deployment_pod_meta(&deployment_id.to_string(), &uid);
            let meta = PodMeta {
                uid,
                name,
                phase,
                restart_count,
            };
            let items = meta.as_redis_items();
            p.hset_multiple(&meta_key, &items).ignore();
            p.expire(&meta_key, ttl).ignore();

            let channel = ChannelNames::deployment_metrics(&deployment_id.to_string());
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

            let deployment_id = deployment_id.unwrap();
            let uid = pod.metadata.uid.as_ref().unwrap().to_string();

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
