use std::collections::HashMap;

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
