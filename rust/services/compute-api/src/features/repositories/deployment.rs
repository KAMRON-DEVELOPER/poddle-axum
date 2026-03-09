use compute_core::{
    formatters::format_resource_name,
    models::{DeploymentRow, DeploymentStatus},
    schemas::{CreateDeploymentRequest, DeploymentSource, UpdateDeploymentRequest},
};
use http_contracts::pagination::schema::Pagination;
use sqlx::types::Json;
use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct DeploymentRepository;

impl DeploymentRepository {
    #[tracing::instrument(
        name = "deployment_repository.get_all_by_project",
        skip_all,
        fields(user_id = %user_id, project_id = %project_id),
        err
    )]
    pub async fn get_all_by_project(
        user_id: &Uuid,
        project_id: &Uuid,
        pagination: &Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<DeploymentRow>, i64), sqlx::Error> {
        // In standard SQL, if you use COUNT(*), the database "collapses" all your rows into a single number.
        // You lose your individual deployment data.
        // OVER() turns the count into a Window Function.
        // It tells Postgres: "Calculate the total count of all rows that match the WHERE clause, but don't collapse them."
        // The exclamation mark (!) is specific to the sqlx::query! macro in Rust. It is called a `Force Non-Null Override`.
        let rows = sqlx::query!(
            r#"
            SELECT
                d.id,
                d.user_id,
                d.project_id,
                d.name,
                d.source AS "source: Json<DeploymentSource>",
                d.port,
                d.desired_replicas,
                d.ready_replicas,
                d.available_replicas,
                d.preset_id,
                d.addon_cpu_millicores,
                d.addon_memory_mb,
                d.vault_secret_path,
                d.secret_keys,
                d.environment_variables AS "environment_variables: Json<Option<HashMap<String, String>>>",
                d.labels AS "labels: Json<Option<HashMap<String, String>>>",
                d.status AS "status: DeploymentStatus",
                d.domain,
                d.subdomain,
                d.service,
                d.created_at,
                d.updated_at,
                COUNT(*) OVER() as "total!"
            FROM deployments d
            INNER JOIN projects p ON d.project_id = p.id
            WHERE p.owner_id = $1 AND d.project_id = $2
            ORDER BY d.created_at DESC
            LIMIT $3
            OFFSET $4
            "#,
            user_id,
            project_id,
            pagination.limit,
            pagination.offset
        )
        .fetch_all(pool)
        .await?;

        // Without that !, your code would have to look like this
        // let total = rows.get(0).map(|r| r.total.unwrap_or(0)).unwrap_or(0);
        // With the !, it's much cleaner
        let total = rows.get(0).map(|r| r.total).unwrap_or(0);

        let deployments = rows
            .into_iter()
            .map(|r| DeploymentRow {
                id: r.id,
                user_id: r.user_id,
                project_id: r.project_id,
                name: r.name,
                source: r.source,
                port: r.port,
                desired_replicas: r.desired_replicas,
                ready_replicas: r.ready_replicas,
                available_replicas: r.available_replicas,
                preset_id: r.preset_id,
                addon_cpu_millicores: r.addon_cpu_millicores,
                addon_memory_mb: r.addon_memory_mb,
                vault_secret_path: r.vault_secret_path,
                secret_keys: r.secret_keys,
                environment_variables: r.environment_variables,
                labels: r.labels,
                status: r.status,
                domain: r.domain,
                subdomain: r.subdomain,
                service: r.service,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();

        Ok((deployments, total))
    }

    #[tracing::instrument(name = "deployment_repository.get_by_id", skip_all, fields(user_id = %user_id, deployment_id = %deployment_id), err)]
    pub async fn get_by_id(
        user_id: &Uuid,
        deployment_id: &Uuid,
        pool: &PgPool,
    ) -> Result<DeploymentRow, sqlx::Error> {
        sqlx::query_as!(
            DeploymentRow,
            r#"
            SELECT
                d.id,
                d.user_id,
                d.project_id,
                d.name,
                d.source AS "source: Json<DeploymentSource>",
                d.port,
                d.desired_replicas,
                d.ready_replicas,
                d.available_replicas,
                d.preset_id,
                d.addon_cpu_millicores,
                d.addon_memory_mb,
                d.vault_secret_path,
                d.secret_keys,
                d.environment_variables AS "environment_variables: Json<Option<HashMap<String, String>>>",
                d.labels AS "labels: Json<Option<HashMap<String, String>>>",
                d.status AS "status: DeploymentStatus",
                d.domain,
                d.subdomain,
                d.service,
                d.created_at,
                d.updated_at
            FROM deployments d
            INNER JOIN projects p ON d.project_id = p.id
            WHERE d.id = $1 AND p.owner_id = $2
            "#,
            deployment_id,
            user_id
        )
        .fetch_one(pool)
        .await
    }

    #[tracing::instrument(name = "deployment_repository.create", skip_all, fields(user_id = %user_id, project_id = %project_id), err)]
    pub async fn create(
        user_id: &Uuid,
        project_id: &Uuid,
        req: CreateDeploymentRequest,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<DeploymentRow, sqlx::Error> {
        let environment_variables =
            serde_json::to_value(&req.environment_variables).unwrap_or(serde_json::json!({}));
        let labels = req
            .labels
            .as_ref()
            .map(|l| serde_json::to_value(l).unwrap());

        let source = serde_json::to_value(req.source).unwrap();

        let id = Uuid::new_v4();
        let name = format_resource_name(&id);

        sqlx::query_as!(
            DeploymentRow,
            r#"
            INSERT INTO deployments (
                id,
                user_id,
                project_id,
                name,
                source,
                port,
                desired_replicas,
                preset_id,
                addon_cpu_millicores,
                addon_memory_mb,
                environment_variables,
                labels,
                domain,
                subdomain,
                service
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING
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
            "#,
            id,
            user_id,
            project_id,
            &req.name,
            &source,
            req.port,
            req.desired_replicas,
            req.preset_id,
            req.addon_cpu_millicores,
            req.addon_memory_mb,
            environment_variables,
            labels,
            req.domain,
            req.subdomain,
            name
        )
        .fetch_one(&mut **tx)
        .await
    }

    #[tracing::instrument(name = "deployment_repository.update_status", skip_all, fields(deployment_id = %deployment_id, status = %status), err)]
    pub async fn update_status(
        deployment_id: &Uuid,
        status: DeploymentStatus,
        pool: &PgPool,
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

    #[tracing::instrument(name = "deployment_repository.update", skip_all, fields(user_id = %user_id, deployment_id = %deployment_id), err)]
    pub async fn update(
        user_id: &Uuid,
        deployment_id: &Uuid,
        req: UpdateDeploymentRequest,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<DeploymentRow, sqlx::Error> {
        let environment_variables = req
            .environment_variables
            .as_ref()
            .map(|e| serde_json::to_value(e).unwrap());
        let labels = req
            .labels
            .as_ref()
            .map(|l| l.as_ref().map(|v| serde_json::to_value(v).unwrap()));

        let source = req
            .source
            .as_ref()
            .map(|s| serde_json::to_value(s).unwrap());

        sqlx::query_as!(
            DeploymentRow,
            r#"
            UPDATE deployments AS d
            SET
                name = COALESCE($3, d.name),
                source = COALESCE($4, d.source),
                port = COALESCE($5, d.port),
                desired_replicas = COALESCE($6, d.desired_replicas),
                preset_id = COALESCE($7, d.preset_id),
                addon_cpu_millicores = COALESCE($8, d.addon_cpu_millicores),
                addon_memory_mb = COALESCE($9, d.addon_memory_mb),
                environment_variables = COALESCE($10, d.environment_variables),
                labels = COALESCE($11, d.labels),
                domain = COALESCE($12, d.domain),
                subdomain = COALESCE($13, d.subdomain)
            FROM projects p
            JOIN deployments d2 ON d2.project_id = p.id
            WHERE
                d.id = d2.id
                AND d.id = $2
                AND p.owner_id = $1
            RETURNING
                d.id,
                d.user_id,
                d.project_id,
                d.name,
                d.source AS "source: Json<DeploymentSource>",
                d.port,
                d.desired_replicas,
                d.ready_replicas,
                d.available_replicas,
                d.preset_id,
                d.addon_cpu_millicores,
                d.addon_memory_mb,
                d.vault_secret_path,
                d.secret_keys,
                d.environment_variables AS "environment_variables: Json<Option<HashMap<String, String>>>",
                d.labels AS "labels: Json<Option<HashMap<String, String>>>",
                d.status AS "status: DeploymentStatus",
                d.domain,
                d.subdomain,
                d.service,
                d.created_at,
                d.updated_at
            "#,
            user_id,
            deployment_id,
            req.name,
            source,
            req.port,
            req.desired_replicas,
            req.preset_id,
            req.addon_cpu_millicores,
            req.addon_memory_mb,
            environment_variables,
            labels.flatten(),
            req.domain,
            req.subdomain
        )
        .fetch_one(&mut **tx)
        .await
    }

    #[tracing::instrument(name = "deployment_repository.delete", skip_all, fields(user_id = %user_id, deployment_id = %deployment_id), err)]
    pub async fn delete(
        user_id: &Uuid,
        deployment_id: &Uuid,
        tx: &mut Transaction<'_, Postgres>,
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
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "deployment_repository.get_prest_id", skip_all, fields(user_id = %user_id, deployment_id = %deployment_id), err)]
    pub async fn get_prest_id(
        user_id: &Uuid,
        deployment_id: &Uuid,
        pool: &PgPool,
    ) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar!(
            r#"
            SELECT d.preset_id
            FROM deployments d
            INNER JOIN projects p ON d.project_id = p.id
            WHERE d.id = $1 AND p.owner_id = $2
            "#,
            deployment_id,
            user_id
        )
        .fetch_one(pool)
        .await
    }
}
