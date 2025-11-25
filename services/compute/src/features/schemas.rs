use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

use crate::features::models::{DeploymentStatus, ResourceSpec};

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

#[derive(Deserialize, Validate, Debug)]
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
