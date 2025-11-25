use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream};
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client, api::ListParams};
use serde::{Deserialize, Serialize};
use shared::utilities::{errors::AppError, jwt::Claims};
use std::convert::Infallible;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use crate::features::repository::DeploymentRepository;
use crate::utilities::app_state::AppState;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMetrics {
    pub deployment_id: String,
    pub status: String,
    pub replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub unavailable_replicas: i32,
    pub pods: Vec<PodMetrics>,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PodMetrics {
    pub name: String,
    pub status: String,
    pub ready: bool,
    pub restarts: i32,
    pub cpu_usage: Option<String>,
    pub memory_usage: Option<String>,
    pub age_seconds: Option<i64>,
}

pub async fn deployment_metrics_stream(
    claims: Claims,
    Path((_project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let user_id = claims.sub;
    let pool = state.database.pool.clone();
    let k8s_client = state.kubernetes.client.clone();

    // Verify ownership
    let deployment = DeploymentRepository::get_by_id(&pool, deployment_id, user_id).await?;

    info!(
        "Starting metrics stream for deployment {} (user {})",
        deployment_id, user_id
    );

    let stream = stream::unfold(
        (k8s_client, deployment.clone()),
        move |(client, deployment)| async move {
            let metrics = fetch_deployment_metrics(&client, &deployment).await;

            match metrics {
                Ok(metrics) => {
                    let event = Event::default().json_data(metrics).unwrap_or_else(|e| {
                        error!("Failed to serialize metrics: {}", e);
                        Event::default().data("error")
                    });

                    tokio::time::sleep(Duration::from_secs(2)).await;
                    Some((Ok(event), (client, deployment)))
                }
                Err(e) => {
                    error!("Failed to fetch metrics: {}", e);
                    let error_event = Event::default()
                        .event("error")
                        .data(format!("Failed to fetch metrics: {}", e));

                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Some((Ok(error_event), (client, deployment)))
                }
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn fetch_deployment_metrics(
    client: &Client,
    deployment: &crate::features::models::Deployment,
) -> Result<DeploymentMetrics, AppError> {
    let namespace = &deployment.cluster_namespace;
    let name = &deployment.cluster_deployment_name;

    // Get K8s Deployment status
    let deployments_api: Api<K8sDeployment> = Api::namespaced(client.clone(), namespace);
    let k8s_deployment = deployments_api
        .get(name)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to get deployment: {}", e)))?;

    let status = k8s_deployment.status.as_ref();
    let replicas = status.and_then(|s| s.replicas).unwrap_or(0);
    let ready_replicas = status.and_then(|s| s.ready_replicas).unwrap_or(0);
    let available_replicas = status.and_then(|s| s.available_replicas).unwrap_or(0);
    let unavailable_replicas = status.and_then(|s| s.unavailable_replicas).unwrap_or(0);

    // Get Pod metrics
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let label_selector = format!("app={}", name);
    let list_params = ListParams::default().labels(&label_selector);

    let pods = pods_api
        .list(&list_params)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to list pods: {}", e)))?;

    let pod_metrics: Vec<PodMetrics> = pods
        .items
        .iter()
        .map(|pod| {
            let pod_status = pod.status.as_ref();
            let phase = pod_status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Check if all containers are ready
            let container_statuses = pod_status.and_then(|s| s.container_statuses.as_ref());
            let ready = container_statuses
                .map(|statuses| statuses.iter().all(|cs| cs.ready))
                .unwrap_or(false);

            // Count restarts
            let restarts = container_statuses
                .and_then(|statuses| statuses.first().map(|cs| cs.restart_count))
                .unwrap_or(0);

            // Calculate pod age
            let age_seconds = pod.metadata.creation_timestamp.as_ref().map(|ts| {
                let now = chrono::Utc::now();
                (now - ts.0).num_seconds()
            });

            PodMetrics {
                name: pod.metadata.name.clone().unwrap_or_default(),
                status: phase,
                ready,
                restarts,
                cpu_usage: None,    // Requires metrics-server
                memory_usage: None, // Requires metrics-server
                age_seconds,
            }
        })
        .collect();

    // Determine overall status
    let overall_status = if ready_replicas == replicas && replicas > 0 {
        "healthy"
    } else if ready_replicas > 0 {
        "degraded"
    } else {
        "unhealthy"
    };

    Ok(DeploymentMetrics {
        deployment_id: deployment.id.to_string(),
        status: overall_status.to_string(),
        replicas,
        ready_replicas,
        available_replicas,
        unavailable_replicas,
        pods: pod_metrics,
        timestamp: chrono::Utc::now().timestamp(),
    })
}
