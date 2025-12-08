use redis::{aio::MultiplexedConnection, pipe};

use shared::{
    models::ResourceSpec,
    schemas::{
        CreateProjectRequest, DeploymentMetrics, MetricPoint, Pagination, UpdateDeploymentRequest,
    },
    utilities::{cache_keys::CacheKeys, errors::AppError},
};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::warn;
use uuid::Uuid;

use shared::{
    models::{Deployment, DeploymentEvent, DeploymentStatus, Project},
    schemas::CreateDeploymentRequest,
};

pub struct ProjectRepository;

impl ProjectRepository {
    pub async fn get_many(
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
        req: CreateProjectRequest,
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
        user_id: Uuid,
        project_id: Uuid,
        pool: &PgPool,
    ) -> Result<(i64, Vec<Deployment>), sqlx::Error> {
        let deployments = sqlx::query_as::<_, Deployment>(
            r#"
                SELECT d.*
                FROM deployments d
                INNER JOIN projects p ON d.project_id = p.id
                WHERE p.owner_id = $1 AND d.project_id = $2
                ORDER BY d.created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        let row = sqlx::query!(
            r#"
                SELECT COUNT(*) as count
                FROM deployments d
                INNER JOIN projects p ON d.project_id = p.id
                WHERE p.owner_id = $1 AND d.project_id = $2
            "#,
            user_id,
            project_id
        )
        .fetch_one(pool)
        .await?;

        let total = row.count.unwrap_or(0);

        Ok((total, deployments))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        user_id: Uuid,
        deployment_id: Uuid,
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
        let namespace = format!("user-{}", &user_id.to_string().replace("-", "")[..16]);
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

    pub async fn update(
        pool: &PgPool,
        deployment_id: Uuid,
        user_id: Uuid,
        req: UpdateDeploymentRequest,
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

pub struct CacheRepository;

impl CacheRepository {
    pub async fn get_deployment_metrics(
        points_count: u64,
        deployment_ids: Vec<Uuid>,
        connection: &mut MultiplexedConnection,
    ) -> Result<Vec<DeploymentMetrics>, AppError> {
        if deployment_ids.is_empty() {
            return Ok(Vec::new());
        }

        let keys = CacheKeys::deployment_metrics(&deployment_ids);
        let cpu_path = format!("$.cpu_history[-{}:]", points_count);
        let mem_path = format!("$.memory_history[-{}:]", points_count);

        let mut p = pipe();
        let _ = p.json_get(&keys, &cpu_path); // JSON.MGET for CPU
        let _ = p.json_get(&keys, &mem_path); // JSON.MGET for Memory 

        // let results: Vec<String> = p
        //     .query_async(connection)
        //     .await
        //     .map_err(|e| AppError::InternalError(format!("Redis pipeline failed: {}", e)))?;

        let (cpu_results, mem_results): (Vec<Option<String>>, Vec<Option<String>>) = p
            .query_async(connection)
            .await
            .map_err(|e| AppError::InternalError(format!("Redis pipeline failed: {}", e)))?;

        warn!("cpu_results: {:?}", cpu_results);
        warn!("mem_results: {:?}", mem_results);

        let parse_metrics = |opt_json: Option<String>| -> Vec<MetricPoint> {
            match opt_json {
                Some(json_str) => {
                    // Parse "[ [...points...] ]" -> inner vector
                    serde_json::from_str::<Vec<Vec<MetricPoint>>>(&json_str)
                        .map(|mut v| v.pop().unwrap_or_default())
                        .unwrap_or_default()
                }
                None => Vec::new(), // Handle missing key gracefully
            }
        };

        let deployment_metrics = cpu_results
            .into_iter()
            .zip(mem_results)
            .map(|(cpu_opt, mem_opt)| DeploymentMetrics {
                cpu_history: parse_metrics(cpu_opt),
                memory_history: parse_metrics(mem_opt),
            })
            .collect();

        // // Add bounds checking
        // if results.len() != 2 {
        //     return Err(AppError::InternalError(format!(
        //         "Expected 2 results from pipeline, got {}",
        //         results.len()
        //     )));
        // }

        // // The first result is CPU data for all keys
        // let cpu_arrays: Vec<Vec<Vec<MetricPoint>>> = serde_json::from_str(&results[0])
        //     .map_err(|e| AppError::InternalError(format!("Failed to parse CPU metrics: {}", e)))?;

        // // The second result is Memory data for all keys
        // let mem_arrays: Vec<Vec<Vec<MetricPoint>>> =
        //     serde_json::from_str(&results[1]).map_err(|e| {
        //         AppError::InternalError(format!("Failed to parse memory metrics: {}", e))
        //     })?;

        // // Combine results
        // let deployment_metrics = cpu_arrays
        //     .into_iter()
        //     .zip(mem_arrays)
        //     .map(|(cpu, mem)| DeploymentMetrics {
        //         cpu_history: cpu.into_iter().next().unwrap_or_default(),
        //         memory_history: mem.into_iter().next().unwrap_or_default(),
        //     })
        //     .collect();

        Ok(deployment_metrics)
    }
}
