use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{DeploymentEventLevel, DeploymentEventRow, DeploymentEventType};

pub struct DeploymentEventRepository;

impl DeploymentEventRepository {
    #[tracing::instrument(
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
