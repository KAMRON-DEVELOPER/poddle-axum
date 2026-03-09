use sqlx::{PgPool, postgres::PgQueryResult};
use tracing::instrument;
use uuid::Uuid;

use crate::models::{
    DeploymentEventLevel, DeploymentEventRow, DeploymentEventType, DeploymentStatus,
};

pub struct DeploymentRepository;

impl DeploymentRepository {
    #[instrument("deployment_repository.update_status", skip_all, fields(deployment_id = %deployment_id, status = %status), err)]
    pub async fn update_status(
        deployment_id: &Uuid,
        status: DeploymentStatus,
        pool: &PgPool,
    ) -> Result<PgQueryResult, sqlx::Error> {
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
}

pub struct DeploymentEventRepository;

impl DeploymentEventRepository {
    #[instrument(
        name = "deployment_event_repository.create",
        skip_all,
        fields(
            project_id = %project_id,
            deployment_id = %deployment_id,
            event_type = %event_type
        ),
        err
    )]
    pub async fn create(
        project_id: &Uuid,
        deployment_id: &Uuid,
        event_type: DeploymentEventType,
        level: DeploymentEventLevel,
        message: Option<&str>,
        pool: &PgPool,
    ) -> Result<DeploymentEventRow, sqlx::Error> {
        sqlx::query_as!(
            DeploymentEventRow,
            r#"
            INSERT INTO deployment_events (project_id, deployment_id, type, level, message)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id,
                project_id,
                deployment_id,
                type AS "event_type: DeploymentEventType",
                level AS "level: DeploymentEventLevel",
                message,
                created_at
            "#,
            project_id,
            deployment_id,
            event_type as DeploymentEventType,
            level as DeploymentEventLevel,
            message
        )
        .fetch_one(pool)
        .await
    }
}
