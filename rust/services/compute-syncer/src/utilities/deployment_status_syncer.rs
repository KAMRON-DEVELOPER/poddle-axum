use compute_core::models::DeploymentStatus;
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::watcher::{Config, Event};
use kube::{Api, Client as KubeClient};
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::error::AppError;

pub async fn start_deployment_status_syncer(
    pool: PgPool,
    client: KubeClient,
    mut redis: MultiplexedConnection,
) -> Result<(), AppError> {
    let watcher_config = Config::default().labels("managed-by=poddle");

    let deployments: Api<K8sDeployment> = Api::all(client.clone());
    let pods: Api<Pod> = Api::all(client.clone());

    let mut deployment_stream = kube::runtime::watcher(deployments, watcher_config.clone()).boxed();
    let mut pod_stream = kube::runtime::watcher(pods, watcher_config).boxed();

    info!("🔍 Starting Kubernetes watchers (filtered by managed-by=poddle)");
    loop {
        tokio::select! {
            Some(event) = deployment_stream.next() => {
                if let Err(e) = handle_deployment_event(event, &pool, &mut redis).await {
                    error!("Failed to handle deployment event: {}", e);
                }
            }
            Some(event) = pod_stream.next() => {
                if let Err(e) = handle_pod_event(event, &pool, &mut redis).await {
                    error!("Failed to handle pod event: {}", e);
                }
            }
            else => {
                warn!("Both watcher streams ended unexpectedly");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_deployment_event(
    event: Result<Event<K8sDeployment>, kube::runtime::watcher::Error>,
    pool: &PgPool,
    connection: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    match event {
        Ok(Event::Apply(deployment)) => {
            // Extract deployment info
            let name = deployment.metadata.name.as_ref().unwrap();
            let namespace = deployment.metadata.namespace.as_ref().unwrap();

            // Check if this is a poddle-managed deployment
            let labels = deployment.metadata.labels.as_ref();
            let deployment_id = labels
                .and_then(|l| l.get("deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if deployment_id.is_none() {
                // Not our deployment, skip
                return Ok(());
            }

            let deployment_id = deployment_id.unwrap();

            // Extract status information
            let spec = deployment.spec.as_ref().unwrap();
            let status = deployment.status.as_ref();

            let desired_replicas = spec.replicas.unwrap_or(1);
            let available_replicas = status.and_then(|s| s.available_replicas).unwrap_or(0);
            let ready_replicas = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let updated_replicas = status.and_then(|s| s.updated_replicas).unwrap_or(0);

            // Determine deployment status based on replica states
            let new_status = determine_deployment_status(
                desired_replicas,
                ready_replicas,
                available_replicas,
                updated_replicas,
            );

            info!(
                "📊 Deployment {} in namespace {}: {:?} ({}/{} ready)",
                name, namespace, new_status, ready_replicas, desired_replicas
            );

            // Update database
            sqlx::query!(
                r#"
                    UPDATE deployments
                    SET status = $2, updated_at = NOW()
                    WHERE id = $1
                "#,
                deployment_id,
                new_status as DeploymentStatus
            )
            .execute(pool)
            .await?;

            // Cache the status in Redis for quick access
            let cache_key = format!("deployment:{}:status", deployment_id);
            let _: () = redis::cmd("SETEX")
                .arg(&cache_key)
                .arg(300) // 5 minute TTL
                .arg(format!("{:?}", new_status))
                .query_async(connection)
                .await?;
        }
        Ok(Event::Delete(deployment)) => {
            let labels = deployment.metadata.labels.as_ref();
            let deployment_id = labels
                .and_then(|l| l.get("deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if let Some(deployment_id) = deployment_id {
                info!(
                    "🗑️ Deployment {} was deleted from Kubernetes",
                    deployment_id
                );

                // Mark as failed/deleted in database
                sqlx::query!(
                    r#"
                    UPDATE deployments
                    SET status = 'failed', updated_at = NOW()
                    WHERE id = $1
                    "#,
                    deployment_id
                )
                .execute(pool)
                .await?;
            }
        }
        Ok(Event::Init) => info!("Watcher started init phase"),
        Ok(Event::InitApply(_)) => {}
        Ok(Event::InitDone) => info!("✅ Watcher initialization complete - State synced"),
        Err(e) => error!("⚠️ Watcher error: {}", e),
    }

    Ok(())
}

async fn handle_pod_event(
    event: Result<Event<Pod>, kube::runtime::watcher::Error>,
    pool: &PgPool,
    connection: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    match event {
        Ok(Event::Apply(pod)) => {
            let labels = pod.metadata.labels.as_ref();
            let deployment_id = labels
                .and_then(|l| l.get("deployment-id"))
                .and_then(|id| Uuid::parse_str(id).ok());

            if deployment_id.is_none() {
                return Ok(());
            }

            let deployment_id = deployment_id.unwrap();
            let pod_name = pod.metadata.name.as_ref().unwrap();

            // Extract pod phase
            let phase = pod.status.as_ref().and_then(|s| s.phase.as_deref());

            // Check container statuses for crashes
            let container_statuses = pod
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref());
            let mut restart_count = 0;
            let mut has_crash_loop = false;

            if let Some(statuses) = container_statuses {
                for status in statuses {
                    restart_count += status.restart_count;

                    // Check for CrashLoopBackOff or ImagePullBackOff
                    if let Some(waiting) = &status.state.as_ref().and_then(|s| s.waiting.as_ref()) {
                        if waiting.reason.as_deref() == Some("CrashLoopBackOff")
                            || waiting.reason.as_deref() == Some("ImagePullBackOff")
                        {
                            has_crash_loop = true;
                        }
                    }
                }
            }

            // If pod is consistently crashing, mark deployment as unhealthy
            if has_crash_loop || restart_count > 5 {
                warn!(
                    "⚠️ Pod {} for deployment {} is unhealthy (restarts: {})",
                    pod_name, deployment_id, restart_count
                );

                sqlx::query!(
                    r#"
                        UPDATE deployments
                        SET status = 'unhealthy', updated_at = NOW()
                        WHERE id = $1
                    "#,
                    deployment_id
                )
                .execute(pool)
                .await?;
            }

            // Cache pod metrics
            let cache_key = format!("deployment:{}:pod:{}", deployment_id, pod_name);
            let pod_data = serde_json::json!({
                "phase": phase,
                "restart_count": restart_count,
                "has_crash_loop": has_crash_loop,
            });

            let _: () = redis::cmd("SETEX")
                .arg(&cache_key)
                .arg(300)
                .arg(pod_data.to_string())
                .query_async(connection)
                .await?;
        }
        Ok(Event::Delete(_)) | Ok(Event::Init) | Ok(Event::InitApply(_)) | Ok(Event::InitDone) => {}
        Err(e) => error!("Pod watcher error: {}", e),
    }

    Ok(())
}

fn determine_deployment_status(
    desired: i32,
    ready: i32,
    available: i32,
    updated: i32,
) -> DeploymentStatus {
    if desired == 0 {
        return DeploymentStatus::Suspended;
    }

    if ready == 0 && available == 0 {
        return DeploymentStatus::Starting;
    }

    if ready == desired && available == desired && updated == desired {
        return DeploymentStatus::Healthy;
    }

    if ready > 0 && ready < desired {
        return DeploymentStatus::Degraded;
    }

    if updated != desired {
        return DeploymentStatus::Updating;
    }

    DeploymentStatus::Unhealthy
}
