use bigdecimal::BigDecimal;
use billing_core::schemas::Money;
use redis::aio::MultiplexedConnection;

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    features::{
        models::DashboardQueryRow,
        schemas::{
            CostOverview, CpuOverview, DashboardResponse, DeploymentOverview, MemoryOverview,
            ResourceOverview,
        },
    },
    services::cache_service::CacheService,
};

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
