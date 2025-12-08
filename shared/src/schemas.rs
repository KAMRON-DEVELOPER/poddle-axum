use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use redis_macros::{FromRedisValue, ToRedisArgs};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

use crate::models::{DeploymentStatus, ResourceSpec};

#[derive(Serialize, Debug)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Pagination {
    #[serde(default = "default_offset")]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_offset() -> i64 {
    0
}

fn default_limit() -> i64 {
    20
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ProjectPageQuery {
    #[serde(default = "project_page_minutes")]
    pub minutes: u64,
}

fn project_page_minutes() -> u64 {
    0
}

// ============================================
// PROJECT SCHEMAS
// ============================================

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

// ============================================
// RESPONSE WRAPPERS
// ============================================

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
}

// ============================================
// DEPLOYMENT SCHEMAS
// ============================================

#[derive(Clone, Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeploymentRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    #[validate(length(min = 1, max = 500))]
    pub image: String,

    #[validate(range(min = 1, max = 65535))]
    pub port: i32,

    #[validate(range(min = 1, max = 10))]
    pub replicas: i32,

    pub secrets: Option<HashMap<String, String>>,

    pub environment_variables: Option<HashMap<String, String>>,

    pub resources: ResourceSpec,

    pub labels: Option<HashMap<String, String>>,

    #[validate(length(min = 3, max = 63), regex(path = *SUBDOMAIN))]
    pub subdomain: Option<String>,

    #[validate(length(min = 3, max = 253), regex(path = *CUSTOM_DOMAIN))]
    pub custom_domain: Option<String>,
}

static SUBDOMAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$").unwrap());

static CUSTOM_DOMAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([a-z0-9]+(-[a-z0-9]+)*\.)+[a-z]{2,}$").unwrap());

#[derive(Deserialize, Validate, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeploymentRequest {
    pub name: Option<String>,
    pub image: Option<String>,
    pub port: Option<i32>,
    #[validate(range(min = 0, max = 100))]
    pub replicas: Option<i32>,
    pub resources: Option<ResourceSpec>,
    pub labels: Option<Option<HashMap<String, String>>>,
    pub secrets: Option<HashMap<String, String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub subdomain: Option<String>,
    pub custom_domain: Option<String>,
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
    pub vault_secret_path: Option<String>,
    pub secret_keys: Option<Vec<String>>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub replicas: i32,
    pub resources: ResourceSpec,
    pub labels: Option<HashMap<String, String>>,
    pub status: DeploymentStatus,
    pub cluster_namespace: String,
    pub cluster_deployment_name: String,
    pub subdomain: Option<String>,
    pub custom_domain: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub cpu_history: Vec<MetricPoint>,
    pub memory_history: Vec<MetricPoint>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEventResponse {
    pub id: Uuid,
    pub event_type: String,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============================================
// RABBITMQ MESSAGE TYPES
// ============================================

/// Message sent to compute.create queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeploymentMessage {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,

    pub name: String,
    pub image: String,
    pub replicas: i32,
    pub port: i32,
    pub environment_variables: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
    pub resources: ResourceSpec,
    pub labels: Option<HashMap<String, String>>,
    pub subdomain: Option<String>,
    pub custom_domain: Option<String>,
    pub timestamp: i64,
}

/// Message sent to compute.scale queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeploymentMessage {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,

    pub name: Option<String>,
    pub image: Option<String>,
    pub replicas: Option<i32>,
    pub port: Option<i32>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
    pub resources: Option<ResourceSpec>,
    pub labels: Option<Option<HashMap<String, String>>>,
    pub subdomain: Option<String>,
    pub custom_domain: Option<String>,
    pub timestamp: i64,
}

/// Message sent to compute.delete queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDeploymentMessage {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
    pub timestamp: i64,
}

// ============================================
// POD METRICS
// ============================================

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetricPoint {
    pub ts: i64,
    pub v: f64,
}

#[derive(FromRedisValue, ToRedisArgs, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PodMetrics {
    pub name: String,
    pub phase: PodPhase,
    pub restarts: u32,

    pub cpu_history: Vec<MetricPoint>,
    pub memory_history: Vec<MetricPoint>,

    pub started_at: Option<i64>,
}

// ============================================
// DEPLOYMENT METRICS
// ============================================

#[derive(FromRedisValue, ToRedisArgs, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMetrics {
    pub cpu_history: Vec<MetricPoint>,
    pub memory_history: Vec<MetricPoint>,
}
