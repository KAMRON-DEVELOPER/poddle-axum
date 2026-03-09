use http_contracts::pagination::schema::Pagination;

use sqlx::PgPool;
use uuid::Uuid;

use crate::features::models::{DashboardEventQueryRow, ProjectEventQueryRow};
use compute_core::models::{DeploymentEventLevel, DeploymentEventType};

pub struct DeploymentEventRepository;

impl DeploymentEventRepository {
    #[tracing::instrument(name = "deployment_event_repository.get_many_by_owner", skip_all, fields(user_id = %user_id), err)]
    pub async fn get_many_by_owner(
        user_id: &Uuid,
        p: &Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<DashboardEventQueryRow>, i64), sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                de.id,
                de.project_id,
                p.name AS project_name,
                de.deployment_id,
                d.name AS deployment_name,
                de.type AS "event_type: DeploymentEventType",
                de.level AS "level: DeploymentEventLevel",
                de.message,
                de.created_at,
                COUNT(*) OVER() AS "total!"
            FROM deployment_events de
            JOIN projects p ON de.project_id = p.id
            JOIN deployments d ON de.deployment_id = d.id
            WHERE p.owner_id = $1
            ORDER BY de.created_at DESC
            LIMIT $2
            OFFSET $3
            "#,
            user_id,
            p.limit,
            p.offset
        )
        .fetch_all(pool)
        .await?;

        let total = rows.get(0).map(|r| r.total).unwrap_or(0);

        let data = rows
            .into_iter()
            .map(|r| DashboardEventQueryRow {
                id: r.id,
                project_id: r.project_id,
                project_name: r.project_name,
                deployment_id: r.deployment_id,
                deployment_name: r.deployment_name,
                event_type: r.event_type,
                level: r.level,
                message: r.message,
                created_at: r.created_at,
            })
            .collect();

        Ok((data, total))
    }

    #[tracing::instrument(name = "deployment_event_repository.get_many_by_project", skip_all, fields(user_id = %user_id, project_id = %project_id), err)]
    pub async fn get_many_by_project(
        user_id: &Uuid,
        project_id: &Uuid,
        p: &Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<ProjectEventQueryRow>, i64), sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                de.id,
                de.deployment_id,
                d.name AS deployment_name,
                de.type AS "event_type: DeploymentEventType",
                de.level AS "level: DeploymentEventLevel",
                de.message,
                de.created_at,
                COUNT(*) OVER() AS "total!"
            FROM deployment_events de
            JOIN projects p ON de.project_id = p.id
            JOIN deployments d ON de.deployment_id = d.id
            WHERE de.project_id = $1
            AND p.owner_id = $2
            ORDER BY de.created_at DESC
            LIMIT $3
            OFFSET $4
            "#,
            user_id,
            project_id,
            p.limit,
            p.offset
        )
        .fetch_all(pool)
        .await?;

        let total = rows.get(0).map(|r| r.total).unwrap_or(0);

        let data = rows
            .into_iter()
            .map(|r| ProjectEventQueryRow {
                id: r.id,
                deployment_id: r.deployment_id,
                deployment_name: r.deployment_name,
                event_type: r.event_type,
                level: r.level,
                message: r.message,
                created_at: r.created_at,
            })
            .collect();

        Ok((data, total))
    }
}
