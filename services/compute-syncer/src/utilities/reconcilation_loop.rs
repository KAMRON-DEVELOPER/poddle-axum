use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use kube::{Api, Client};
use shared::models::DeploymentStatus;
use shared::utilities::errors::AppError;
use sqlx::PgPool;
use std::time::Duration;
use tracing::{error, info, warn};

/// Periodic reconciliation to catch missed events and fix drift
pub async fn reconciliation_loop(pool: PgPool, client: Client) -> Result<(), AppError> {
    let mut interval = tokio::time::interval(Duration::from_secs(120)); // Every 2 minutes

    info!("🔄 Starting reconciliation loop");

    loop {
        interval.tick().await;

        if let Err(e) = reconcile_all_deployments(&pool, &client).await {
            error!("Reconciliation failed: {}", e);
        }
    }
}

async fn reconcile_all_deployments(pool: &PgPool, client: &Client) -> Result<(), AppError> {
    // Fetch all active deployments from database
    let db_deployments = sqlx::query!(
        r#"
        SELECT id, cluster_namespace, cluster_deployment_name, status as "status: DeploymentStatus", replicas
        FROM deployments
        WHERE status NOT IN ('failed', 'suspended')
        "#
    )
    .fetch_all(pool)
    .await?;

    info!("🔍 Reconciling {} deployments", db_deployments.len());

    for db_deployment in db_deployments {
        let namespace = &db_deployment.cluster_namespace;
        let name = &db_deployment.cluster_deployment_name;
        let deployment_id = db_deployment.id;

        // Try to fetch from Kubernetes
        let deployments_api: Api<K8sDeployment> = Api::namespaced(client.clone(), namespace);

        match deployments_api.get(name).await {
            Ok(k8s_deployment) => {
                // Deployment exists in K8s, check if status matches
                let spec = k8s_deployment.spec.as_ref().unwrap();
                let status = k8s_deployment.status.as_ref();

                let desired = spec.replicas.unwrap_or(1);
                let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
                let available = status.and_then(|s| s.available_replicas).unwrap_or(0);
                let updated = status.and_then(|s| s.updated_replicas).unwrap_or(0);

                let computed_status = determine_status(desired, ready, available, updated);

                // Check for drift
                if computed_status != db_deployment.status {
                    warn!(
                        "⚠️ Status drift detected for {}: DB={:?}, K8s={:?}",
                        deployment_id, db_deployment.status, computed_status
                    );

                    // Fix the drift
                    sqlx::query!(
                        r#"
                        UPDATE deployments
                        SET status = $2, updated_at = NOW()
                        WHERE id = $1
                        "#,
                        deployment_id,
                        computed_status as DeploymentStatus
                    )
                    .execute(pool)
                    .await?;

                    info!("✅ Fixed status drift for {}", deployment_id);
                }

                // Check replica count drift
                if desired != db_deployment.replicas {
                    warn!(
                        "⚠️ Replica drift for {}: DB={}, K8s={}",
                        deployment_id, db_deployment.replicas, desired
                    );

                    // Update DB to match K8s (K8s is source of truth for actual state)
                    sqlx::query!(
                        r#"
                        UPDATE deployments
                        SET replicas = $2, updated_at = NOW()
                        WHERE id = $1
                        "#,
                        deployment_id,
                        desired
                    )
                    .execute(pool)
                    .await?;
                }
            }
            Err(kube::Error::Api(api_error)) if api_error.code == 404 => {
                // Deployment deleted from K8s but still in DB
                warn!(
                    "⚠️ Deployment {} exists in DB but not in K8s - marking as failed",
                    deployment_id
                );

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
            Err(e) => {
                error!("Failed to check K8s deployment {}: {}", deployment_id, e);
            }
        }
    }

    Ok(())
}

fn determine_status(desired: i32, ready: i32, available: i32, updated: i32) -> DeploymentStatus {
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
        return DeploymentStatus::Scaling;
    }

    DeploymentStatus::Unhealthy
}
