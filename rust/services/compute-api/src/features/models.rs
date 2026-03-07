use bigdecimal::BigDecimal;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Debug)]
pub struct DashboardQueryRow {
    pub projects_count: i64,

    pub total: i64,
    pub queued: i64,
    pub building: i64,
    pub provisioning: i64,
    pub starting: i64,
    pub running: i64,
    pub unhealthy: i64,
    pub degraded: i64,
    pub updating: i64,
    pub suspended: i64,
    pub failed: i64,
    pub build_failed: i64,
    pub image_pull_error: i64,

    pub allocated_cpu_millicores: i64,
    pub allocated_memory_mb: i64,

    pub estimated_monthly_cost: BigDecimal,
}

#[derive(Debug, FromRow)]
pub struct ProjectOverviewQueryRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,

    pub total: i64,
    pub queued: i64,
    pub building: i64,
    pub provisioning: i64,
    pub starting: i64,
    pub running: i64,
    pub unhealthy: i64,
    pub degraded: i64,
    pub updating: i64,
    pub suspended: i64,
    pub failed: i64,
    pub build_failed: i64,
    pub image_pull_error: i64,

    pub allocated_cpu_millicores: i64,
    pub allocated_memory_mb: i64,

    pub estimated_monthly_cost: BigDecimal,
}
