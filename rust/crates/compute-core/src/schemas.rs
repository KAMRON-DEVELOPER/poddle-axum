use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use redis_macros::{FromRedisValue, ToRedisArgs};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

use crate::models::{DeploymentStatus, ResourceSpec};

// -----------------------------------------------
// PROJECT SCHEMAS
// -----------------------------------------------

#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub struct ProjectPageQuery {
    #[serde(default = "project_page_minutes")]
    #[serde_as(as = "DisplayFromStr")]
    pub minutes: u64,
}

fn project_page_minutes() -> u64 {
    5
}

#[derive(Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    #[validate(length(min = 1, max = 36))]
    pub name: String,
    #[validate(length(max = 180))]
    pub description: Option<String>,
}

#[derive(Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    #[validate(length(min = 1, max = 20))]
    pub name: Option<String>,
    #[validate(length(max = 180))]
    pub description: Option<String>,
}

// -----------------------------------------------
// DEPLOYMENT SCHEMAS
// -----------------------------------------------

/// Whole point is we don't implement Display & Debug

#[derive(Clone, Deserialize)]
pub struct SecretString(pub String);

#[derive(Clone, Deserialize, Serialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImagePullSecret {
    pub server: String,
    pub username: String,
    pub secret: String,
}

#[derive(Clone, Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeploymentRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: String,
    #[validate(length(min = 1, max = 500))]
    pub image: String,
    pub image_pull_secret: Option<ImagePullSecret>,
    #[validate(range(min = 1, max = 65535))]
    pub port: i32,
    #[validate(range(min = 1, max = 25))]
    pub desired_replicas: i32,
    pub preset_id: Uuid,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub secrets: Option<HashMap<String, String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    #[validate(length(min = 3, max = 253), regex(path = *DOMAIN))]
    pub domain: Option<String>,
    #[validate(length(min = 3, max = 63), regex(path = *SUBDOMAIN))]
    pub subdomain: Option<String>,
}

static SUBDOMAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$").unwrap());

static DOMAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([a-z0-9]+(-[a-z0-9]+)*\.)+[a-z]{2,}$").unwrap());

#[derive(Deserialize, Validate, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeploymentRequest {
    pub name: Option<String>,
    pub image: Option<String>,
    pub image_pull_secret: Option<ImagePullSecret>,
    pub port: Option<i32>,
    #[validate(range(min = 0, max = 25))]
    pub desired_replicas: Option<i32>,
    pub preset_id: Option<Uuid>,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub secrets: Option<HashMap<String, String>>,
    pub secrets_to_delete: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<Option<HashMap<String, String>>>,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub image: String,
    pub port: i32,
    pub desired_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub preset_id: Uuid,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub secret_keys: Option<Vec<String>>,
    pub vault_secret_path: Option<String>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    pub status: DeploymentStatus,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub pods: Vec<Pod>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentsResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub image: String,
    pub port: i32,
    pub desired_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub preset_id: Uuid,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub secret_keys: Option<Vec<String>>,
    pub vault_secret_path: Option<String>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    pub status: DeploymentStatus,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metrics: Vec<MetricSnapshot>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEventResponse {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub event_type: String,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// -----------------------------------------------
// RABBITMQ MESSAGE TYPES
// -----------------------------------------------

/// Message sent to `compute.create` queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeploymentMessage {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
    pub name: String,
    pub image: String,
    pub image_pull_secret: Option<ImagePullSecret>,
    pub port: i32,
    pub desired_replicas: i32,
    pub preset_id: Uuid,
    pub resource_spec: ResourceSpec,
    pub secrets: Option<HashMap<String, String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
}

/// Message sent to `compute.scale` queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeploymentMessage {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
    pub name: Option<String>,
    pub image: Option<String>,
    pub image_pull_secret: Option<ImagePullSecret>,
    pub port: Option<i32>,
    pub desired_replicas: Option<i32>,
    pub preset_id: Option<Uuid>,
    pub resource_spec: Option<ResourceSpec>,
    pub secrets: Option<HashMap<String, String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub labels: Option<Option<HashMap<String, String>>>,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub timestamp: i64,
}

/// Message sent to `compute.delete` queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDeploymentMessage {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
    pub timestamp: i64,
}

// -----------------------------------------------
// POD & DEPLOYMENT METRICS
// -----------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct MetricSnapshot {
    pub ts: i64,
    pub cpu: f64,
    pub memory: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pod {
    pub name: String,
    pub metrics: Vec<MetricSnapshot>,
}

#[derive(FromRedisValue, ToRedisArgs, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MetricHistory {
    #[serde(default)]
    pub snapshots: Vec<MetricSnapshot>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMetricUpdate {
    pub id: String,
    pub snapshot: MetricSnapshot,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PodMetricUpdate {
    pub name: String,
    pub snapshot: MetricSnapshot,
}
