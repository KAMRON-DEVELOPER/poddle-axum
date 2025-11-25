use shared::schemas::Pagination;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::features::{
    models::{Deployment, DeploymentEvent, DeploymentStatus, Project},
    schemas::CreateDeploymentRequest,
};

pub struct ProjectRepository;

impl ProjectRepository {
    pub async fn get_many_by_user_id(
        pool: &PgPool,
        user_id: Uuid,
        pagination: Pagination,
    ) -> Result<(Vec<Project>, i64), sqlx::Error> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
                SELECT id, owner_id, name, description, created_at, updated_at
                FROM projects
                WHERE owner_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(pool)
        .await?;

        let row = sqlx::query!(
            r#"
                SELECT COUNT(*) as count
                FROM projects d
                WHERE owner_id = $1
            "#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        let total = row.count.unwrap_or(0);

        Ok((projects, total))
    }

    pub async fn get_one_by_id(
        pool: &PgPool,
        project_id: Uuid,
        user_id: Uuid,
    ) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
                SELECT id, owner_id, name, description, created_at, updated_at
                FROM projects
                WHERE id = $1 AND owner_id = $2
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
    }

    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
                INSERT INTO projects (owner_id, name, description)
                VALUES ($1, $2, $3)
                RETURNING id, owner_id, name, description, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(description)
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &PgPool,
        project_id: Uuid,
        user_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
                UPDATE projects
                SET name = COALESCE($3, name),
                    description = COALESCE($4, description)
                WHERE id = $1 AND owner_id = $2
                RETURNING id, owner_id, name, description, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .bind(name)
        .bind(description)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, project_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
                DELETE FROM projects
                WHERE id = $1 AND owner_id = $2
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}

pub struct DeploymentRepository;

impl DeploymentRepository {
    pub async fn get_user_namespace(user_id: Uuid) -> String {
        format!("user-{}", &user_id.to_string().replace("-", "")[..16])
    }

    pub async fn get_all_by_project(
        pool: &PgPool,
        project_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Deployment>, sqlx::Error> {
        sqlx::query_as::<_, Deployment>(
            r#"
                SELECT d.*
                FROM deployments d
                INNER JOIN projects p ON d.project_id = p.id
                WHERE d.project_id = $1 AND p.owner_id = $2
                ORDER BY d.created_at DESC
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn get_by_id(
        pool: &PgPool,
        deployment_id: Uuid,
        user_id: Uuid,
    ) -> Result<Deployment, sqlx::Error> {
        sqlx::query_as::<_, Deployment>(
            r#"
                SELECT d.*
                FROM deployments d
                INNER JOIN projects p ON d.project_id = p.id
                WHERE d.id = $1 AND p.owner_id = $2
            "#,
        )
        .bind(deployment_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
    }

    pub async fn create(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        project_id: Uuid,
        req: CreateDeploymentRequest,
    ) -> Result<Deployment, sqlx::Error> {
        sqlx::query_as::<_, Deployment>(
            r#"
                INSERT INTO deployments (
                    user_id, project_id, name, image, env_vars, replicas,
                    resources, labels, cluster_namespace, cluster_deployment_name
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .bind(req.name)
        .bind(req.image)
        .bind(req.env_vars)
        .bind(req.replicas)
        .bind(req.resources)
        .bind(req.labels)
        .bind(req.cluster_namespace)
        .bind(req.cluster_deployment_name)
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn update_status(
        pool: &PgPool,
        deployment_id: Uuid,
        status: DeploymentStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
                UPDATE deployments
                SET status = $2
                WHERE id = $1
            "#,
        )
        .bind(deployment_id)
        .bind(status)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_replicas(
        pool: &PgPool,
        deployment_id: Uuid,
        user_id: Uuid,
        replicas: i32,
    ) -> Result<Deployment, sqlx::Error> {
        sqlx::query_as::<_, Deployment>(
            r#"
                UPDATE deployments d
                SET replicas = $3
                FROM projects p
                WHERE d.id = $1 AND d.project_id = p.id AND p.owner_id = $2
                RETURNING d.*
            "#,
        )
        .bind(deployment_id)
        .bind(user_id)
        .bind(replicas)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(
        pool: &PgPool,
        deployment_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
                DELETE FROM deployments d
                USING projects p
                WHERE d.id = $1 AND d.project_id = p.id AND p.owner_id = $2
            "#,
        )
        .bind(deployment_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}

pub struct DeploymentEventRepository;

impl DeploymentEventRepository {
    pub async fn create(
        pool: &PgPool,
        deployment_id: Uuid,
        event_type: &str,
        message: Option<&str>,
    ) -> Result<DeploymentEvent, sqlx::Error> {
        sqlx::query_as::<_, DeploymentEvent>(
            r#"
                INSERT INTO deployment_events (deployment_id, event_type, message)
                VALUES ($1, $2, $3)
                RETURNING *
            "#,
        )
        .bind(deployment_id)
        .bind(event_type)
        .bind(message)
        .fetch_one(pool)
        .await
    }

    pub async fn get_recent_by_deployment(
        pool: &PgPool,
        deployment_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DeploymentEvent>, sqlx::Error> {
        sqlx::query_as::<_, DeploymentEvent>(
            r#"
                SELECT * FROM deployment_events
                WHERE deployment_id = $1
                ORDER BY created_at DESC
                LIMIT $2
            "#,
        )
        .bind(deployment_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}
