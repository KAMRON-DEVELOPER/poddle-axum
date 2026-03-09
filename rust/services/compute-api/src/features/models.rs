use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use compute_core::models::{DeploymentEventLevel, DeploymentEventType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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

#[derive(FromRow, Debug)]
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

#[derive(FromRow, Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DashboardEventQueryRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub deployment_id: Uuid,
    pub deployment_name: String,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub event_type: DeploymentEventType,
    pub level: DeploymentEventLevel,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEventQueryRow {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub deployment_name: String,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub event_type: DeploymentEventType,
    pub level: DeploymentEventLevel,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}
