use std::collections::HashMap;

use billing_core::schemas::Money;
use chrono::{DateTime, Utc};
use compute_core::{models::DeploymentStatus, schemas::DeploymentSource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallbackParams {
    pub installation_id: i64,
    pub setup_action: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct LokiResponse {
    pub status: String,
    pub data: LokiData,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LokiData {
    pub result_type: String,
    pub result: Vec<LokiStreamResult>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct LokiTailResponse {
    pub streams: Vec<LokiStreamResult>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct LokiStreamResult {
    pub stream: HashMap<String, String>,
    pub values: Vec<[String; 2]>,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: Option<String>,
    pub stream: Option<String>,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct LogResponse {
    pub entries: Vec<LogEntry>,
}

#[derive(Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentOut {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub preset_id: Uuid,
    pub name: String,
    pub source: DeploymentSource,
    pub port: i32,
    pub desired_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub vault_secret_path: Option<String>,
    pub secret_keys: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    pub status: DeploymentStatus,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub service: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentOverview {
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
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CpuOverview {
    pub used_millicores: Option<f64>,
    pub allocated_millicores: i64,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub used_mb: Option<f64>,
    pub allocated_mb: i64,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResourceOverview {
    pub cpu_overview: CpuOverview,
    pub memory_overview: MemoryOverview,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CostOverview {
    pub spent_this_month: Option<Money>,
    pub estimated_monthly_cost: Money,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResponse {
    pub projects_count: i64,
    pub deployment_overview: DeploymentOverview,
    pub resource_overview: ResourceOverview,
    pub cost_overview: CostOverview,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOverviewResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub deployment_overview: DeploymentOverview,
    pub resource_overview: ResourceOverview,
    pub cost_overview: CostOverview,
}
