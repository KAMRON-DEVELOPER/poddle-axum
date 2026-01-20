use compute_core::{
    cache_keys::CacheKeys,
    models::{Deployment, DeploymentEvent, DeploymentStatus, Project, ResourceSpec},
    schemas::{
        CreateDeploymentRequest, CreateProjectRequest, DeploymentMetrics, MetricSnapshot,
        UpdateDeploymentRequest,
    },
};
use http_contracts::pagination::schema::Pagination;
use redis::{aio::MultiplexedConnection, pipe};
use sqlx::types::Json;
use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

pub struct ProjectRepository;

impl ProjectRepository {
    #[tracing::instrument(
        name = "project_repository.get_many",
        skip(user_id, pagination, pool),
        err
    )]
    pub async fn get_many(
        user_id: Uuid,
        pagination: Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<Project>, i64), sqlx::Error> {
        // let projects = sqlx::query_as::<_, Project>(
        //     r#"
        //     SELECT id, owner_id, name, description, created_at, updated_at
        //     FROM projects
        //     WHERE owner_id = $1
        //     ORDER BY created_at DESC
        //     LIMIT $2
        //     OFFSET $3
        //     "#,
        // )
        // .bind(user_id)
        // .bind(pagination.limit)
        // .bind(pagination.offset)
        // .fetch_all(pool)
        // .await?;

        // let row = sqlx::query!(
        //     r#"
        //         SELECT COUNT(*) as count
        //         FROM projects d
        //         WHERE owner_id = $1
        //     "#,
        //     user_id
        // )
        // .fetch_one(pool)
        // .await?;

        // let total = row.count.unwrap_or(0);

        let rows = sqlx::query!(
            r#"
            SELECT 
                id, owner_id, name, description, created_at, updated_at,
                COUNT(*) OVER() as total
            FROM projects
            WHERE owner_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            OFFSET $3
            "#,
            user_id,
            pagination.limit,
            pagination.offset
        )
        .fetch_all(pool)
        .await?;

        let total = rows.get(0).map(|r| r.total.unwrap_or(0)).unwrap_or(0);

        let projects = rows
            .into_iter()
            .map(|r| Project {
                id: r.id,
                owner_id: r.owner_id,
                name: r.name,
                description: r.description,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();

        Ok((projects, total))
    }

    #[tracing::instrument(name = "project_repository.get_one_by_id", skip(pool), err)]
    pub async fn get_one_by_id(
        user_id: Uuid,
        project_id: Uuid,
        pool: &PgPool,
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

    #[tracing::instrument(name = "project_repository.create", skip(req, pool), err)]
    pub async fn create(
        user_id: Uuid,
        req: CreateProjectRequest,
        pool: &PgPool,
    ) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (owner_id, name, description)
            VALUES ($1, $2, $3)
            RETURNING id, owner_id, name, description, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(req.name)
        .bind(req.description)
        .fetch_one(pool)
        .await
    }

    #[tracing::instrument(name = "project_repository.update", skip(name, description, pool), err)]
    pub async fn update(
        user_id: Uuid,
        project_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        pool: &PgPool,
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

    #[tracing::instrument(name = "project_repository.delete", skip(pool), err)]
    pub async fn delete(user_id: Uuid, project_id: Uuid, pool: &PgPool) -> Result<(), sqlx::Error> {
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
    #[tracing::instrument(name = "deployment_repository.get_user_namespace")]
    pub fn get_user_namespace(user_id: &Uuid) -> String {
        // as_simple() produces a hyphen-free, lowercase hex representation
        let short = user_id.as_simple().to_string();
        format!("user-{}", &short[..16])
    }

    #[tracing::instrument(
        name = "deployment_repository.get_all_by_project",
        skip(pagination, pool),
        err
    )]
    pub async fn get_all_by_project(
        user_id: Uuid,
        project_id: Uuid,
        pagination: Pagination,
        pool: &PgPool,
    ) -> Result<(i64, Vec<Deployment>), sqlx::Error> {
        // let deployments = sqlx::query_as::<_, Deployment>(
        //     r#"
        //     SELECT d.*
        //     FROM deployments d
        //     INNER JOIN projects p ON d.project_id = p.id
        //     WHERE p.owner_id = $1 AND d.project_id = $2
        //     ORDER BY d.created_at DESC
        //     LIMIT $2
        //     OFFSET $3
        //     "#,
        // )
        // .bind(user_id)
        // .bind(project_id)
        // .bind(pagination.limit)
        // .bind(pagination.offset)
        // .fetch_all(pool)
        // .await?;

        // let row = sqlx::query!(
        //     r#"
        //         SELECT COUNT(*) as count
        //         FROM deployments d
        //         INNER JOIN projects p ON d.project_id = p.id
        //         WHERE p.owner_id = $1 AND d.project_id = $2
        //     "#,
        //     user_id,
        //     project_id
        // )
        // .fetch_one(pool)
        // .await?;

        // let total = row.count.unwrap_or(0);

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
                d.cluster_namespace,
                d.cluster_deployment_name,
                d.name,
                d.image,
                d.port,
                d.replicas,
                d.resources AS "resources: Json<ResourceSpec>",
                d.vault_secret_path,
                d.secret_keys,
                d.environment_variables AS "environment_variables: Json<Option<HashMap<String, String>>>",
                d.labels AS "labels: Json<Option<HashMap<String, String>>>",
                d.status AS "status: DeploymentStatus",
                d.subdomain,
                d.custom_domain,
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
            .map(|r| Deployment {
                id: r.id,
                user_id: r.user_id,
                project_id: r.project_id,
                cluster_namespace: r.cluster_namespace,
                cluster_deployment_name: r.cluster_deployment_name,
                name: r.name,
                image: r.image,
                port: r.port,
                replicas: r.replicas,
                resources: r.resources,
                vault_secret_path: r.vault_secret_path,
                secret_keys: r.secret_keys,
                environment_variables: r.environment_variables,
                labels: r.labels,
                status: r.status,
                subdomain: r.subdomain,
                custom_domain: r.custom_domain,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();

        Ok((total, deployments))
    }

    #[tracing::instrument(name = "deployment_repository.get_by_id", skip(pool), err)]
    pub async fn get_by_id(
        user_id: Uuid,
        deployment_id: Uuid,
        pool: &PgPool,
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

    #[tracing::instrument(name = "deployment_repository.create", skip(req, tx), err)]
    pub async fn create(
        user_id: Uuid,
        project_id: Uuid,
        req: CreateDeploymentRequest,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Deployment, sqlx::Error> {
        let namespace = DeploymentRepository::get_user_namespace(&user_id);
        let deployment_name = format!(
            "{}-{}",
            req.name.to_lowercase().replace("_", "-"),
            &Uuid::new_v4().to_string()[..8]
        );

        let environment_variables =
            serde_json::to_value(&req.environment_variables).unwrap_or(serde_json::json!({}));
        let resources = serde_json::to_value(&req.resources)
            .unwrap_or_else(|_| serde_json::to_value(&ResourceSpec::default()).unwrap());
        let labels = req
            .labels
            .as_ref()
            .map(|l| serde_json::to_value(l).unwrap());

        sqlx::query_as::<_, Deployment>(
            r#"
            INSERT INTO deployments (
                user_id, project_id, name, image, replicas, port, environment_variables, resources,
                labels, subdomain, cluster_namespace, cluster_deployment_name
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .bind(&req.name)
        .bind(&req.image)
        .bind(req.replicas)
        .bind(req.port)
        .bind(environment_variables)
        .bind(resources)
        .bind(labels)
        .bind(&req.subdomain)
        .bind(namespace)
        .bind(deployment_name)
        .fetch_one(&mut **tx)
        .await
    }

    #[tracing::instrument(name = "deployment_repository.update_status", skip(pool), err)]
    pub async fn update_status(
        deployment_id: Uuid,
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

    #[tracing::instrument(name = "deployment_repository.update", skip(req, tx), err)]
    pub async fn update(
        user_id: Uuid,
        deployment_id: Uuid,
        req: UpdateDeploymentRequest,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Deployment, sqlx::Error> {
        let resources = req
            .resources
            .as_ref()
            .map(|r| serde_json::to_value(r).unwrap());
        let labels = req
            .labels
            .as_ref()
            .map(|l| l.as_ref().map(|v| serde_json::to_value(v).unwrap()));
        let env_vars = req
            .environment_variables
            .as_ref()
            .map(|e| serde_json::to_value(e).unwrap());

        sqlx::query_as::<_, Deployment>(
            r#"
            UPDATE deployments d
            SET
                name = COALESCE($3, d.name),
                image = COALESCE($4, d.image),
                port = COALESCE($5, d.port),
                replicas = COALESCE($6, d.replicas),
                resources = COALESCE($7, d.resources),
                labels = COALESCE($8, d.labels),
                environment_variables = COALESCE($9, d.environment_variables),
                subdomain = COALESCE($10, d.subdomain),
                custom_domain = COALESCE($11, d.custom_domain)
            FROM projects p
            WHERE d.id = $1 AND d.project_id = p.id AND p.owner_id = $2
            RETURNING d.*
            "#,
        )
        .bind(deployment_id)
        .bind(user_id)
        .bind(&req.name)
        .bind(&req.image)
        .bind(req.port)
        .bind(req.replicas)
        .bind(resources)
        .bind(labels.flatten())
        .bind(env_vars)
        .bind(&req.subdomain)
        .bind(&req.custom_domain)
        .fetch_one(&mut **tx)
        .await
    }

    #[tracing::instrument(name = "deployment_repository.delete", skip(tx), err)]
    pub async fn delete(
        user_id: Uuid,
        deployment_id: Uuid,
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
}

pub struct DeploymentEventRepository;

impl DeploymentEventRepository {
    #[tracing::instrument(name = "deployment_event_repository.create", skip(message, pool), err)]
    pub async fn create(
        deployment_id: Uuid,
        event_type: &str,
        message: Option<&str>,
        pool: &PgPool,
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

    #[tracing::instrument(
        name = "deployment_event_repository.get_recent_by_deployment",
        skip(pool),
        err
    )]
    pub async fn get_recent_by_deployment(
        deployment_id: Uuid,
        limit: i64,
        pool: &PgPool,
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

pub struct CacheRepository;

impl CacheRepository {
    #[tracing::instrument(
        name = "cache_repository.get_deployment_metrics",
        skip(points_count, deployment_ids, connection),
        err
    )]
    pub async fn get_deployment_metrics(
        points_count: u64,
        deployment_ids: Vec<&str>,
        connection: &mut MultiplexedConnection,
    ) -> Result<Vec<DeploymentMetrics>, AppError> {
        // Safety check for Redis syntax
        if deployment_ids.is_empty() {
            return Ok(Vec::new());
        }

        let keys = CacheKeys::deployments_metrics(&deployment_ids);
        let path = format!("$.history[-{}:]", points_count);

        let mut p = pipe();
        // Runs JSON.GET if key is singular, JSON.MGET if there are multiple keys
        let _ = p.json_get(&keys, &path);

        // Avoid auto-switch behavior:
        // p.cmd("JSON.MGET").arg(&keys).arg(&path);
        // This makes result shape predictable.

        // We expect `results` to have length 1 (because we sent 1 command: JSON.MGET)
        let results: Vec<Option<Vec<String>>> = p
            .query_async(connection)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Redis pipeline failed: {}", e)))?;

        // Extract MGET Results safely
        // Take the first result, flatten the Option, and default to empty Vec on failure
        let mget_results = results.into_iter().next().flatten().unwrap_or_default();

        // Map to Domain Objects with Length Guarantee
        // We iterate over the INPUT length (deployment_ids), not the Redis result length.
        // This ensures that if Redis returns fewer items or fails, we pad with empty metrics
        // rather than dropping the deployment from the UI.
        let deployment_metrics = (0..deployment_ids.len())
            .map(|i| {
                // Try to get the JSON string at index `i`
                let history = mget_results
                    .get(i)
                    // If found, try to parse it
                    .and_then(|json_str| serde_json::from_str::<Vec<MetricSnapshot>>(json_str).ok())
                    // If index missing OR parsing failed, return empty history
                    .unwrap_or_default();

                DeploymentMetrics { history }
            })
            .collect();

        Ok(deployment_metrics)
    }
}
