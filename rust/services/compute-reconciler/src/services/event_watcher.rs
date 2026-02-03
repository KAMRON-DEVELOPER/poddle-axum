use compute_core::channel_names::ChannelNames;
use compute_core::determiners::determine_deployment_status;
use compute_core::event::{ComputeEvent, EventLevel};
use compute_core::models::DeploymentStatus;
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::watcher::{Config, Event};
use kube::{Api, Client};
use redis::AsyncTypedCommands;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tracing::{Instrument, error, info, info_span, warn};
use uuid::Uuid;

use crate::error::AppError;

pub async fn event_watcher(
    pool: PgPool,
    mut redis: MultiplexedConnection,
    client: Client,
) -> Result<(), AppError> {
    let watcher_config = Config::default().labels("managed-by=poddle");

    let deployment: Api<K8sDeployment> = Api::all(client.clone());
    let pod: Api<Pod> = Api::all(client.clone());

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
                if let Err(e) = handle_pod_event(event, &pool, &mut redis).await {
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

            let message = ComputeEvent::StatusUpdate {
                id: &deployment_id,
                status: new_status,
            };
            send_message(&project_id, message, con).await?
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

            let message = ComputeEvent::StatusUpdate {
                id: &deployment_id,
                status: DeploymentStatus::Deleted,
            };
            send_message(&project_id, message, con).await?
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
    event: Result<Event<Pod>, kube::runtime::watcher::Error>,
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
            let pod_name = pod.metadata.name.as_ref().unwrap();

            // Extract pod phase
            // let phase_str = pod
            //     .status
            //     .as_ref()
            //     .and_then(|s| s.phase.as_deref())
            //     .unwrap_or("Unknown");
            // let phase = match phase_str {
            //     "Pending" => PodPhase::Pending,
            //     "Running" => PodPhase::Running,
            //     "Succeeded" => PodPhase::Succeeded,
            //     "Failed" => PodPhase::Failed,
            //     _ => PodPhase::Unknown,
            // };

            // Health Analysis
            let mut restart_count = 0;
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
                    pod_name, ns, reason, restart_count
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
                    let message = ComputeEvent::SystemMessage {
                        deployment_id: &deployment_id,
                        message: format!("Deployment is crashing: {}", reason),
                        level: EventLevel::Error,
                    };
                    send_message(&project_id, message, con).await?
                }
            }
        }
        Ok(Event::Delete(_)) | Ok(Event::Init) | Ok(Event::InitApply(_)) | Ok(Event::InitDone) => {}
        Err(e) => error!("Pod watcher error: {}", e),
    }

    Ok(())
}

async fn when_affacted_rows_zero(
    project_id: &Uuid,
    deployment_id: &Uuid,
    con: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    {
        let channel = ChannelNames::project_metrics(&project_id.to_string());
        let message = ComputeEvent::SystemMessage {
            deployment_id: deployment_id,
            level: EventLevel::Error,
            message: "Internal server error".to_string(),
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

async fn send_message(
    project_id: &Uuid,
    msg: ComputeEvent<'_>,
    con: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    let channel = ChannelNames::project_metrics(&project_id.to_string());
    con.publish(channel, msg)
        .instrument(info_span!("pubsub.status_update"))
        .await?;

    Ok(())
}
