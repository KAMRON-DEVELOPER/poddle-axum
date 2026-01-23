use compute_core::models::{Deployment, DeploymentStatus};
use sqlx::{PgPool, postgres::PgQueryResult};
use tracing::instrument;
use uuid::Uuid;

use crate::error::AppError;

pub struct DeploymentRepository;

impl DeploymentRepository {
    #[instrument("deployment_repository.update_status", skip_all, fields(deployment_id = %deployment_id, status = %status), err)]
    pub async fn update_status(
        deployment_id: &Uuid,
        status: DeploymentStatus,
        pool: &PgPool,
    ) -> Result<PgQueryResult, AppError> {
        Ok(sqlx::query!(
            r#"
            UPDATE deployments
            SET status = $1
            WHERE id = $2"#,
            status as DeploymentStatus,
            deployment_id
        )
        .execute(pool)
        .await?)
    }

    #[instrument("deployment_repository.update_replicas", skip_all, fields(deployment_id = %deployment_id, desired_replicas = %desired_replicas), err)]
    pub async fn update_replicas(
        deployment_id: &Uuid,
        desired_replicas: i32,
        pool: &PgPool,
    ) -> Result<PgQueryResult, AppError> {
        Ok(sqlx::query!(
            r#"
            UPDATE deployments
            SET desired_replicas = $1
            WHERE id = $2
            "#,
            desired_replicas,
            deployment_id
        )
        .execute(pool)
        .await?)
    }

    #[instrument("deployment_repository.get_one_by_id", skip_all, fields(deployment_id = %deployment_id), err)]
    pub async fn get_one_by_id(
        deployment_id: &Uuid,
        pool: &PgPool,
    ) -> Result<Deployment, sqlx::Error> {
        sqlx::query_as::<_, Deployment>(
            r#"
            SELECT cluster_namespace, cluster_deployment_name
            FROM deployments
            WHERE id = $1
            "#,
        )
        .bind(deployment_id)
        .fetch_one(pool)
        .await
    }
}
