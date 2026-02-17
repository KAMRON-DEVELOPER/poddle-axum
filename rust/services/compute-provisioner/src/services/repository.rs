use crate::error::AppError;
use compute_core::{
    models::{Deployment, DeploymentStatus},
    schemas::DeploymentSource,
};
use sqlx::{PgPool, postgres::PgQueryResult, types::Json};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

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

    #[instrument("deployment_repository.update_replicas", skip_all, fields(deployment_id = %id, desired_replicas = %desired_replicas), err)]
    pub async fn update_replicas(
        id: &Uuid,
        desired_replicas: i32,
        pool: &PgPool,
    ) -> Result<PgQueryResult, sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE deployments
            SET desired_replicas = $1
            WHERE id = $2
            "#,
            desired_replicas,
            id
        )
        .execute(pool)
        .await
    }

    #[instrument("deployment_repository.get_one_by_id", skip_all, fields(deployment_id = %id), err)]
    pub async fn get_by_id(id: &Uuid, pool: &PgPool) -> Result<Deployment, sqlx::Error> {
        sqlx::query_as!(
            Deployment,
            r#"
            SELECT
                id,
                user_id,
                project_id,
                name,
                source AS "source: Json<DeploymentSource>",
                port,
                desired_replicas,
                ready_replicas,
                available_replicas,
                preset_id,
                addon_cpu_millicores,
                addon_memory_mb,
                vault_secret_path,
                secret_keys,
                environment_variables AS "environment_variables: Json<Option<HashMap<String, String>>>",
                labels AS "labels: Json<Option<HashMap<String, String>>>",
                status AS "status: DeploymentStatus",
                domain,
                subdomain,
                service,
                created_at,
                updated_at
            FROM deployments
            WHERE id = $1
            "#,
            id
        )
        .fetch_one(pool)
        .await
    }

    #[instrument("deployment_repository.get_vault_secret_path", skip_all, fields(deployment_id = %id), err)]
    pub async fn get_vault_secret_path(
        id: &Uuid,
        pool: &PgPool,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar!(
            r#"
            SELECT vault_secret_path FROM deployments
            WHERE id = $1
            "#,
            id
        )
        .fetch_one(pool)
        .await
    }

    #[instrument("deployment_repository.set_vault_path", skip_all, fields(deployment_id = %id), err)]
    pub async fn set_vault_secret_path(
        id: &Uuid,
        path: &str,
        pool: &PgPool,
    ) -> Result<PgQueryResult, sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE deployments
            SET vault_secret_path = $1
            WHERE id = $2
            "#,
            path,
            id
        )
        .execute(pool)
        .await
    }

    #[instrument("deployment_repository.set_secret_keys", skip_all, fields(deployment_id = %id), err)]
    pub async fn set_secret_keys(
        id: &Uuid,
        keys: Vec<String>,
        pool: &PgPool,
    ) -> Result<PgQueryResult, sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE deployments
            SET secret_keys = $1
            WHERE id = $2
            "#,
            &keys,
            id
        )
        .execute(pool)
        .await
    }
}
