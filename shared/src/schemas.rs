use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use redis_macros::{FromRedisValue, ToRedisArgs};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

use crate::models::{DeploymentStatus, ResourceSpec};
use crate::utilities::errors::AppError;

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

impl Pagination {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.offset < 0 {
            return Err(AppError::ValidationError(
                "Offset must be positive".to_string(),
            ));
        }

        if self.limit < 0 {
            return Err(AppError::ValidationError("Limit must positive".to_string()));
        } else if self.limit == 0 {
            return Err(AppError::ValidationError(
                "Limit must not be zero!".to_string(),
            ));
        }

        if self.limit > 100 {
            return Err(AppError::ValidationError(
                "Limit cannot exceed 100".to_string(),
            ));
        }

        Ok(())
    }
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
// DEPLOYMENT SCHEMAS
// ============================================

#[derive(Clone, Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeploymentRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    #[validate(length(min = 1, max = 500))]
    pub image: String,

    #[validate(range(min = 1, max = 10))]
    pub replicas: i32,

    /// Port that the container exposes
    #[validate(range(min = 1, max = 65535))]
    pub port: i32,

    /// Environment variables (non-sensitive)
    pub env_vars: Option<HashMap<String, String>>,

    /// Secret environment variables (will be encrypted)
    pub secrets: Option<HashMap<String, String>>,

    /// Resource limits
    pub resources: Option<ResourceSpec>,

    /// Custom labels for the deployment
    pub labels: Option<HashMap<String, String>>,

    /// Subdomain for the deployment (optional, auto-generated if not provided)
    #[validate(length(min = 3, max = 63), regex(path = *SUBDOMAIN))]
    pub subdomain: Option<String>,
}

static SUBDOMAIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$").unwrap());

#[derive(Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeploymentRequest {
    #[validate(range(min = 0, max = 10))]
    pub replicas: Option<i32>,

    pub env_vars: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
    pub resources: Option<ResourceSpec>,
}

#[derive(Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScaleDeploymentRequest {
    #[validate(range(min = 0, max = 10))]
    pub replicas: i32,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub image: String,
    pub status: DeploymentStatus,
    pub replicas: i32,
    pub resources: ResourceSpec,
    pub external_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentDetailResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub image: String,
    pub status: DeploymentStatus,
    pub replicas: i32,
    pub ready_replicas: Option<i32>,
    pub resources: ResourceSpec,
    pub env_vars: HashMap<String, String>,
    pub secret_keys: Vec<String>, // Only return keys, not values
    pub labels: Option<HashMap<String, String>>,
    pub external_url: Option<String>,
    pub cluster_namespace: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
// RESPONSE WRAPPERS
// ============================================

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
}

impl MessageResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

// ============================================
// POD METRICS
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRedisValue)]
#[serde(rename_all = "camelCase")]
pub enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRedisValue, ToRedisArgs)]
#[serde(rename_all = "camelCase")]
pub struct PodMetrics {
    pub pod_name: String,
    pub phase: PodPhase,
    pub cpu_millicores: f64,
    pub memory_bytes: u64,
    pub restarts: u32,
    pub started_at: Option<i64>,
}

// ============================================
// DEPLOYMENT METRICS
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMetrics {
    pub deployment_id: String,
    pub status: DeploymentStatus,
    pub replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub unavailable_replicas: i32,
    pub pods: Vec<PodMetrics>,
    pub timestamp: i64,
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

    pub env_vars: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
    pub resources: ResourceSpec,
    pub labels: Option<HashMap<String, String>>,
    pub subdomain: Option<String>,

    pub timestamp: i64,
}

impl CreateDeploymentMessage {
    pub fn from_request(
        deployment_id: Uuid,
        user_id: Uuid,
        project_id: Uuid,
        req: CreateDeploymentRequest,
    ) -> Self {
        Self {
            deployment_id,
            user_id,
            project_id,
            name: req.name,
            image: req.image,
            replicas: req.replicas,
            port: req.port,
            env_vars: req.env_vars.unwrap_or_default(),
            secrets: req.secrets.unwrap_or_default(),
            resources: req.resources.unwrap_or_default(),
            labels: req.labels,
            subdomain: req.subdomain,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// Message sent to compute.scale queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScaleDeploymentMessage {
    pub deployment_id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub replicas: i32,
    pub timestamp: i64,
}

/// Message sent to compute.delete queue
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDeploymentMessage {
    pub deployment_id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub timestamp: i64,
}
