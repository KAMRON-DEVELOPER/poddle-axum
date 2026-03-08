use bigdecimal::BigDecimal;
use billing_core::schemas::Money;
use compute_core::{
    formatters::format_resource_name,
    models::{
        DeploymentEventLevel, DeploymentEventRow, DeploymentEventType, DeploymentRow,
        DeploymentStatus, PresetRow, ProjectRow,
    },
    schemas::{
        CreateDeploymentRequest, CreateProjectRequest, DeploymentSource, UpdateDeploymentRequest,
    },
};
use http_contracts::pagination::schema::Pagination;
use redis::aio::MultiplexedConnection;
use sqlx::{Executor, types::Json};
use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    features::{
        models::{DashboardQueryRow, ProjectOverviewQueryRow},
        schemas::{
            CostOverview, CpuOverview, DashboardResponse, DeploymentOverview, MemoryOverview,
            ProjectOverviewResponse, ResourceOverview,
        },
    },
    services::cache_service::CacheService,
};

// --------------------------------------------------------------------------
// DashboardRepository
// --------------------------------------------------------------------------

pub struct DashboardRepository;

impl DashboardRepository {
    pub async fn get_dashboard(
        user_id: &Uuid,
        pool: &PgPool,
        con: &mut MultiplexedConnection,
    ) -> Result<DashboardResponse, AppError> {
        let row = sqlx::query_as!(
            DashboardQueryRow,
            r#"
            WITH latest_addon_price AS (
                SELECT
                    cpu_monthly_unit_price,
                    memory_monthly_unit_price,
                    currency
                FROM addon_prices
                ORDER BY created_at DESC
                LIMIT 1
            )
            SELECT
                COALESCE((SELECT COUNT(id) FROM projects WHERE owner_id = $1), 0)::BIGINT AS "projects_count!",

                COUNT(d.id)::BIGINT AS "total!",
                COUNT(d.id) FILTER (WHERE d.status = 'queued')::BIGINT AS "queued!",
                COUNT(d.id) FILTER (WHERE d.status = 'building')::BIGINT AS "building!",
                COUNT(d.id) FILTER (WHERE d.status = 'provisioning')::BIGINT AS "provisioning!",
                COUNT(d.id) FILTER (WHERE d.status = 'starting')::BIGINT AS "starting!",
                COUNT(d.id) FILTER (WHERE d.status = 'running')::BIGINT AS "running!",
                COUNT(d.id) FILTER (WHERE d.status = 'unhealthy')::BIGINT AS "unhealthy!",
                COUNT(d.id) FILTER (WHERE d.status = 'degraded')::BIGINT AS "degraded!",
                COUNT(d.id) FILTER (WHERE d.status = 'updating')::BIGINT AS "updating!",
                COUNT(d.id) FILTER (WHERE d.status = 'suspended')::BIGINT AS "suspended!",
                COUNT(d.id) FILTER (WHERE d.status = 'failed')::BIGINT AS "failed!",
                COUNT(d.id) FILTER (WHERE d.status = 'build_failed')::BIGINT AS "build_failed!",
                COUNT(d.id) FILTER (WHERE d.status = 'image_pull_error')::BIGINT AS "image_pull_error!",

                COALESCE(SUM(
                    (p.cpu_millicores + COALESCE(d.addon_cpu_millicores, 0)) * d.desired_replicas
                ), 0)::BIGINT AS "allocated_cpu_millicores!",

                COALESCE(SUM(
                    (p.memory_mb + COALESCE(d.addon_memory_mb, 0)) * d.desired_replicas
                ), 0)::BIGINT AS "allocated_memory_mb!",

                COALESCE(SUM(
                    CASE
                        WHEN d.status IN ('provisioning', 'starting', 'running', 'unhealthy', 'degraded', 'updating')
                        THEN (
                            p.monthly_price
                            + COALESCE(d.addon_cpu_millicores, 0)::NUMERIC * lap.cpu_monthly_unit_price
                            + COALESCE(d.addon_memory_mb, 0)::NUMERIC * lap.memory_monthly_unit_price
                        ) * d.desired_replicas::NUMERIC
                        ELSE 0::NUMERIC
                    END
                ), 0::NUMERIC) AS "estimated_monthly_cost!"
            FROM deployments d
            JOIN presets p ON p.id = d.preset_id
            CROSS JOIN latest_addon_price lap
            WHERE d.user_id = $1
            AND d.status != 'deleted'
            "#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        let deployment_ids = sqlx::query_scalar!(
            r#"
            SELECT id
            FROM deployments
            WHERE user_id = $1
            AND status != 'deleted'
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        let id_strings: Vec<String> = deployment_ids.iter().map(Uuid::to_string).collect();
        let id_refs: Vec<&str> = id_strings.iter().map(String::as_str).collect();

        let metrics = CacheService::get_latest_deployments_metrics(id_refs, con).await?;

        let used_millicores = metrics
            .iter()
            .filter_map(|m| m.as_ref())
            .map(|m| m.cpu)
            .sum();
        let used_mb = metrics
            .iter()
            .filter_map(|m| m.as_ref())
            .map(|m| m.memory)
            .sum();
        let has_any_metric = metrics.iter().any(|m| m.is_some());

        Ok(DashboardResponse {
            projects_count: row.projects_count,
            deployment_overview: DeploymentOverview {
                total: row.total,
                queued: row.queued,
                building: row.building,
                provisioning: row.provisioning,
                starting: row.starting,
                running: row.running,
                unhealthy: row.unhealthy,
                degraded: row.degraded,
                updating: row.updating,
                suspended: row.suspended,
                failed: row.failed,
                build_failed: row.build_failed,
                image_pull_error: row.image_pull_error,
            },
            resource_overview: ResourceOverview {
                cpu_overview: CpuOverview {
                    used_millicores: has_any_metric.then_some(used_millicores),
                    allocated_millicores: row.allocated_cpu_millicores,
                },
                memory_overview: MemoryOverview {
                    used_mb: has_any_metric.then_some(used_mb),
                    allocated_mb: row.allocated_memory_mb,
                },
            },
            cost_overview: CostOverview {
                spent_this_month: Some(Money {
                    amount: BigDecimal::from(0),
                    currency: "UZS".to_string(),
                }),
                estimated_monthly_cost: Money {
                    amount: row.estimated_monthly_cost,
                    currency: "UZS".to_string(),
                },
            },
        })
    }
}

// --------------------------------------------------------------------------
// ProjectRepository
// --------------------------------------------------------------------------
pub struct ProjectRepository;

impl ProjectRepository {
    #[tracing::instrument(name = "project_repository.get_many_overviews", skip_all, err)]
    pub async fn get_many_overviews(
        user_id: &Uuid,
        pagination: &Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<ProjectOverviewResponse>, i64), sqlx::Error> {
        let projects_count = sqlx::query_scalar!(
            "SELECT COUNT(id) FROM projects WHERE owner_id = $1",
            user_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        if projects_count == 0 {
            return Ok((vec![], 0));
        }

        let rows = sqlx::query_as!(
            ProjectOverviewQueryRow,
            r#"
            WITH latest_addon_price AS (
                SELECT
                    cpu_monthly_unit_price,
                    memory_monthly_unit_price,
                    currency
                FROM addon_prices
                ORDER BY created_at DESC
                LIMIT 1
            )
            SELECT
                prj.id AS "id!",
                prj.name AS "name!",
                prj.description AS "description?",

                COUNT(d.id)::BIGINT AS "total!",
                COUNT(d.id) FILTER (WHERE d.status = 'queued')::BIGINT AS "queued!",
                COUNT(d.id) FILTER (WHERE d.status = 'building')::BIGINT AS "building!",
                COUNT(d.id) FILTER (WHERE d.status = 'provisioning')::BIGINT AS "provisioning!",
                COUNT(d.id) FILTER (WHERE d.status = 'starting')::BIGINT AS "starting!",
                COUNT(d.id) FILTER (WHERE d.status = 'running')::BIGINT AS "running!",
                COUNT(d.id) FILTER (WHERE d.status = 'unhealthy')::BIGINT AS "unhealthy!",
                COUNT(d.id) FILTER (WHERE d.status = 'degraded')::BIGINT AS "degraded!",
                COUNT(d.id) FILTER (WHERE d.status = 'updating')::BIGINT AS "updating!",
                COUNT(d.id) FILTER (WHERE d.status = 'suspended')::BIGINT AS "suspended!",
                COUNT(d.id) FILTER (WHERE d.status = 'failed')::BIGINT AS "failed!",
                COUNT(d.id) FILTER (WHERE d.status = 'build_failed')::BIGINT AS "build_failed!",
                COUNT(d.id) FILTER (WHERE d.status = 'image_pull_error')::BIGINT AS "image_pull_error!",

                COALESCE(SUM(
                    (p.cpu_millicores + COALESCE(d.addon_cpu_millicores, 0)) * d.desired_replicas
                ), 0)::BIGINT AS "allocated_cpu_millicores!",

                COALESCE(SUM(
                    (p.memory_mb + COALESCE(d.addon_memory_mb, 0)) * d.desired_replicas
                ), 0)::BIGINT AS "allocated_memory_mb!",

                COALESCE(SUM(
                    CASE
                        WHEN d.status IN ('provisioning', 'starting', 'running', 'unhealthy', 'degraded', 'updating')
                        THEN (
                            p.monthly_price
                            + COALESCE(d.addon_cpu_millicores, 0)::NUMERIC * lap.cpu_monthly_unit_price
                            + COALESCE(d.addon_memory_mb, 0)::NUMERIC * lap.memory_monthly_unit_price
                        ) * d.desired_replicas::NUMERIC
                        ELSE 0::NUMERIC
                    END
                ), 0::NUMERIC) AS "estimated_monthly_cost!"

            FROM projects prj
            LEFT JOIN deployments d
                ON d.project_id = prj.id
                AND d.user_id = $1
                AND d.status != 'deleted'
            LEFT JOIN presets p
                ON p.id = d.preset_id
            CROSS JOIN latest_addon_price lap
            WHERE prj.owner_id = $1
            GROUP BY prj.id, prj.name, prj.description
            ORDER BY prj.created_at DESC
            LIMIT $2 OFFSET $3;
            "#,
            user_id,
            pagination.limit as i64,
            pagination.offset as i64
        )
        .fetch_all(pool)
        .await?;

        let data = rows
            .into_iter()
            .map(|row| {
                ProjectOverviewResponse {
                    id: row.id,
                    name: row.name,
                    description: row.description,
                    deployment_overview: DeploymentOverview {
                        total: row.total,
                        queued: row.queued,
                        building: row.building,
                        provisioning: row.provisioning,
                        starting: row.starting,
                        running: row.running,
                        unhealthy: row.unhealthy,
                        degraded: row.degraded,
                        updating: row.updating,
                        suspended: row.suspended,
                        failed: row.failed,
                        build_failed: row.build_failed,
                        image_pull_error: row.image_pull_error,
                    },
                    resource_overview: ResourceOverview {
                        cpu_overview: CpuOverview {
                            used_millicores: None, // Will fill via Redis later
                            allocated_millicores: row.allocated_cpu_millicores,
                        },
                        memory_overview: MemoryOverview {
                            used_mb: None, // Will fill via Redis later
                            allocated_mb: row.allocated_memory_mb,
                        },
                    },
                    cost_overview: CostOverview {
                        spent_this_month: Some(Money {
                            amount: BigDecimal::from(0),
                            currency: "UZS".to_string(),
                        }),
                        estimated_monthly_cost: Money {
                            amount: row.estimated_monthly_cost,
                            currency: "UZS".to_string(),
                        },
                    },
                }
            })
            .collect();

        Ok((data, projects_count))
    }

    #[tracing::instrument(name = "project_repository.get_many", skip_all, err)]
    pub async fn get_many(
        user_id: &Uuid,
        pagination: &Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<ProjectRow>, i64), sqlx::Error> {
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
            .map(|r| ProjectRow {
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
        user_id: &Uuid,
        project_id: &Uuid,
        pool: &PgPool,
    ) -> Result<ProjectRow, sqlx::Error> {
        sqlx::query_as!(
            ProjectRow,
            r#"
            SELECT id, owner_id, name, description, created_at, updated_at
            FROM projects
            WHERE id = $1 AND owner_id = $2
            "#,
            project_id,
            user_id
        )
        .fetch_one(pool)
        .await
    }

    #[tracing::instrument(name = "project_repository.create", skip(req, pool), err)]
    pub async fn create(
        user_id: &Uuid,
        req: CreateProjectRequest,
        pool: &PgPool,
    ) -> Result<ProjectRow, sqlx::Error> {
        sqlx::query_as!(
            ProjectRow,
            r#"
            INSERT INTO projects (owner_id, name, description)
            VALUES ($1, $2, $3)
            RETURNING id, owner_id, name, description, created_at, updated_at
            "#,
            user_id,
            req.name,
            req.description
        )
        .fetch_one(pool)
        .await
    }

    #[tracing::instrument(name = "project_repository.update", skip(name, description, pool), err)]
    pub async fn update(
        user_id: &Uuid,
        project_id: &Uuid,
        name: Option<&str>,
        description: Option<&str>,
        pool: &PgPool,
    ) -> Result<ProjectRow, sqlx::Error> {
        sqlx::query_as!(
            ProjectRow,
            r#"
            UPDATE projects
            SET name = COALESCE($3, name),
                description = COALESCE($4, description)
            WHERE id = $1 AND owner_id = $2
            RETURNING id, owner_id, name, description, created_at, updated_at
            "#,
            project_id,
            user_id,
            name,
            description
        )
        .fetch_one(pool)
        .await
    }

    #[tracing::instrument(name = "project_repository.delete", skip(pool), err)]
    pub async fn delete(
        user_id: &Uuid,
        project_id: &Uuid,
        pool: &PgPool,
    ) -> Result<(), sqlx::Error> {
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

// --------------------------------------------------------------------------
// DeploymentPresetRepository
// --------------------------------------------------------------------------
pub struct DeploymentPresetRepository;

impl DeploymentPresetRepository {
    #[tracing::instrument(name = "deployment_preset_repository.get_by_id", skip(executor), err)]
    pub async fn get_by_id<'e, E>(preset_id: &Uuid, executor: E) -> Result<PresetRow, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            PresetRow,
            r#"
            SELECT *
            FROM presets
            WHERE id = $1
            "#,
            preset_id
        )
        .fetch_one(executor)
        .await
    }
}

// --------------------------------------------------------------------------
// DeploymentRepository
// --------------------------------------------------------------------------
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

pub struct DeploymentEventRepository;

impl DeploymentEventRepository {
    #[tracing::instrument(name = "deployment_event_repository.get_many", skip_all, fields(project_id = %project_id), err)]
    pub async fn get_many(
        project_id: &Uuid,
        p: &Pagination,
        pool: &PgPool,
    ) -> Result<Vec<DeploymentEventRow>, sqlx::Error> {
        sqlx::query_as!(
            DeploymentEventRow,
            r#"
            SELECT
                id,
                project_id,
                deployment_id,
                type AS "event_type: DeploymentEventType",
                level AS "level: DeploymentEventLevel",
                message,
                created_at
            FROM deployment_events
            WHERE project_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            OFFSET $3
            "#,
            project_id,
            p.limit,
            p.offset
        )
        .fetch_all(pool)
        .await
    }

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
            event_type,
            level,
            message
        )
        .fetch_one(pool)
        .await
    }
}
