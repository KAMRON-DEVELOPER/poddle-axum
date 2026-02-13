use compute_core::{
    determiners::determine_deployment_status,
    formatters::{format_namespace, format_resource_name},
    models::DeploymentStatus,
};
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use kube::{Api, Client};
use sqlx::PgPool;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::error::AppError;

/// Periodic reconciliation to catch missed events and fix drift
pub async fn start_reconciliation_loop(
    reconciliation_interval_secs: u64,
    pool: PgPool,
    client: Client,
) -> Result<(), AppError> {
    let mut interval = tokio::time::interval(Duration::from_secs(reconciliation_interval_secs));

    info!(
        "🔄 Starting reconciliation loop, interval: {}",
        reconciliation_interval_secs
    );

    loop {
        interval.tick().await;

        if let Err(e) = reconcile_deployments(&pool, &client).await {
            error!(error = %e, "❌ Reconciliation failed");
        }
    }
}

#[tracing::instrument("reconcile_deployments", skip_all, err)]
async fn reconcile_deployments(pool: &PgPool, client: &Client) -> Result<(), AppError> {
    // Fetch all active deployments from database
    let db_deployments = sqlx::query!(
        r#"
        SELECT id, user_id, status as "status: DeploymentStatus", desired_replicas, ready_replicas, available_replicas
        FROM deployments
        WHERE status NOT IN ('failed', 'suspended', 'image_pull_error')
        "#
    )
    .fetch_all(pool)
    .await?;

    info!(
        "🔍 Reconcilation started with {} deployments",
        db_deployments.len()
    );

    let start = std::time::Instant::now();

    for db_deployment in db_deployments {
        let namespace = &format_namespace(&db_deployment.user_id);
        let name = &format_resource_name(&db_deployment.id);
        let id = db_deployment.id;

        // Try to fetch from Kubernetes
        let deployment_api: Api<K8sDeployment> = Api::namespaced(client.clone(), namespace);

        match deployment_api.get(name).await {
            Ok(k8s_deployment) => {
                let spec = k8s_deployment.spec.as_ref().unwrap();
                let status = k8s_deployment.status.as_ref();

                let desired = spec.replicas.unwrap_or(1);
                let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
                let available = status.and_then(|s| s.available_replicas).unwrap_or(0);
                let updated = status.and_then(|s| s.updated_replicas).unwrap_or(0);

                let computed_status =
                    determine_deployment_status(desired, ready, available, updated);

                // Check for drift
                if computed_status != db_deployment.status {
                    warn!(
                        id = %id,
                        "⚠️ Status drift detected: DB={:?}, K8s={:?}",
                        db_deployment.status, computed_status
                    );

                    // Fix the drift
                    sqlx::query!(
                        r#"
                        UPDATE deployments
                        SET status = $2
                        WHERE id = $1
                        "#,
                        id,
                        computed_status as DeploymentStatus
                    )
                    .execute(pool)
                    .await?;

                    info!(id = %id, "✅ Fixed status drift");
                }

                // Check replica count drift
                if desired != db_deployment.desired_replicas {
                    warn!(
                        id = %id,
                        "⚠️ Replica drift detected: DB={}, K8s={}",
                        db_deployment.desired_replicas, desired
                    );

                    // Update DB to match K8s (K8s is source of truth for actual state)
                    sqlx::query!(
                        r#"
                        UPDATE deployments
                        SET desired_replicas = $2
                        WHERE id = $1
                        "#,
                        id,
                        desired
                    )
                    .execute(pool)
                    .await?;
                }
            }
            Err(kube::Error::Api(e)) if e.code == 404 => {
                // Deployment deleted from K8s but still in DB
                warn!(
                    id = %id,
                    "⚠️ Deployment {} exists in DB but not in K8s - marking as failed",
                    id
                );

                sqlx::query!(
                    r#"
                        UPDATE deployments
                        SET status = 'failed', updated_at = NOW()
                        WHERE id = $1
                    "#,
                    id
                )
                .execute(pool)
                .await?;
            }
            Err(e) => {
                error!(error = %e, id = %id, "❌ Failed to check K8s deployment");
            }
        }
    }

    info!(
        elapsed = start.elapsed().as_millis(),
        "✅ Reconcilation done",
    );

    Ok(())
}
