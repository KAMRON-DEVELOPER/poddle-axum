use std::collections::HashMap;

use bigdecimal::BigDecimal;
use billing_core::schemas::Money;
use compute_core::{models::ProjectRow, schemas::CreateProjectRequest};
use http_contracts::pagination::schema::Pagination;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    features::{
        models::ProjectOverviewQueryRow,
        schemas::{
            CostOverview, CpuOverview, DeploymentOverview, MemoryOverview, ProjectOverviewResponse,
            ResourceOverview,
        },
    },
};

pub struct ProjectRepository;

impl ProjectRepository {
    #[tracing::instrument(name = "project_repository.get_many_overviews", skip_all, err)]
    pub async fn get_many_overviews(
        user_id: &Uuid,
        pagination: &Pagination,
        pool: &PgPool,
        con: &mut MultiplexedConnection,
    ) -> Result<(Vec<ProjectOverviewResponse>, i64), AppError> {
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
            pagination.limit,
            pagination.offset
        )
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Ok((vec![], projects_count));
        }

        let project_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();

        let deployment_pairs = sqlx::query!(
            r#"
        SELECT id, project_id
        FROM deployments
        WHERE user_id = $1
        AND project_id = ANY($2)
        AND status != 'deleted'
        "#,
            user_id,
            &project_ids
        )
        .fetch_all(pool)
        .await?;

        let mut deployment_ids = Vec::with_capacity(deployment_pairs.len());
        let mut deployment_project_ids = Vec::with_capacity(deployment_pairs.len());

        for row in deployment_pairs {
            deployment_ids.push(row.id.to_string());
            deployment_project_ids.push(row.project_id);
        }

        let deployment_id_refs: Vec<&str> = deployment_ids.iter().map(String::as_str).collect();

        let latest_metrics =
            crate::services::cache_service::CacheService::get_latest_deployments_metrics(
                deployment_id_refs,
                con,
            )
            .await?;

        let mut usage_by_project: HashMap<Uuid, (f64, f64, bool)> = HashMap::new();

        for (project_id, maybe_metric) in deployment_project_ids
            .into_iter()
            .zip(latest_metrics.into_iter())
        {
            let entry = usage_by_project
                .entry(project_id)
                .or_insert((0.0, 0.0, false));

            if let Some(metric) = maybe_metric {
                entry.0 += metric.cpu;
                entry.1 += metric.memory;
                entry.2 = true;
            }
        }

        let data = rows
            .into_iter()
            .map(|row| {
                let (used_cpu, used_memory, has_metrics) = usage_by_project
                    .get(&row.id)
                    .copied()
                    .unwrap_or((0.0, 0.0, false));

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
                            used_millicores: has_metrics.then_some(used_cpu),
                            allocated_millicores: row.allocated_cpu_millicores,
                        },
                        memory_overview: MemoryOverview {
                            used_mb: has_metrics.then_some(used_memory),
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

    #[tracing::instrument(name = "project_repository.get_one_overview", skip_all, fields(user_id = %user_id, project_id = %project_id), err)]
    pub async fn get_one_overview(
        user_id: &Uuid,
        project_id: &Uuid,
        pool: &PgPool,
        con: &mut MultiplexedConnection,
    ) -> Result<ProjectOverviewResponse, AppError> {
        let row = sqlx::query_as!(
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
            WHERE prj.id = $2 AND prj.owner_id = $1
            GROUP BY prj.id, prj.name, prj.description;
            "#,
            user_id,
            project_id
        )
        .fetch_one(pool)
        .await?;

        let deployment_ids = sqlx::query_scalar!(
            r#"
        SELECT id
        FROM deployments
        WHERE user_id = $1
        AND project_id = $2
        AND status != 'deleted'
        "#,
            user_id,
            project_id
        )
        .fetch_all(pool)
        .await?;

        let id_strings: Vec<String> = deployment_ids.iter().map(Uuid::to_string).collect();
        let id_refs: Vec<&str> = id_strings.iter().map(String::as_str).collect();

        let metrics = crate::services::cache_service::CacheService::get_latest_deployments_metrics(
            id_refs, con,
        )
        .await?;

        let used_millicores: f64 = metrics
            .iter()
            .filter_map(|m| m.as_ref())
            .map(|m| m.cpu)
            .sum();

        let used_mb: f64 = metrics
            .iter()
            .filter_map(|m| m.as_ref())
            .map(|m| m.memory)
            .sum();

        let has_any_metric = metrics.iter().any(|m| m.is_some());

        let data = ProjectOverviewResponse {
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
        };

        Ok(data)
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
            SET name = COALESCE($3, name), description = COALESCE($4, description)
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
