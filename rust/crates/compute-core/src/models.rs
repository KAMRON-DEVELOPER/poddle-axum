use std::collections::HashMap;

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type, types::Json};
use uuid::Uuid;

use crate::schemas::DeploymentSource;

// ---------------------------------------------
// ENUMS
// ---------------------------------------------

#[derive(Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "deployment_status", rename_all = "snake_case")]
pub enum DeploymentStatus {
    Building,
    Queued,
    Provisioning,
    Starting,
    Running,
    Unhealthy,
    Degraded,
    Updating,
    Suspended,
    Failed,
    BuildFailed,
    Deleted,
    ImagePullError,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Building => write!(f, "building"),
            Self::Queued => write!(f, "queued"),
            Self::Provisioning => write!(f, "provisioning"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Updating => write!(f, "scaling"),
            Self::Suspended => write!(f, "suspended"),
            Self::Failed => write!(f, "failed"),
            Self::BuildFailed => write!(f, "build_failed"),
            Self::Deleted => write!(f, "deleted"),
            Self::ImagePullError => write!(f, "image_pull_error"),
        }
    }
}

// ---------------------------------------------
// MODELS
// ---------------------------------------------

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub cpu_millicores: i32,
    pub memory_mb: i32,
    pub currency: String,
    pub monthly_price: BigDecimal,
    pub hourly_price: BigDecimal,
    pub max_addon_cpu_millicores: i32,
    pub max_addon_memory_mb: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AddonPrices {
    pub id: Uuid,
    pub cpu_monthly_unit_price: BigDecimal,
    pub cpu_hourly_unit_price: BigDecimal,
    pub memory_monthly_unit_price: BigDecimal,
    pub memory_hourly_unit_price: BigDecimal,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub preset_id: Uuid,
    pub name: String,
    pub source: Json<DeploymentSource>,
    pub port: i32,
    pub desired_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub vault_secret_path: Option<String>,
    pub secret_keys: Option<Vec<String>>,
    pub environment_variables: Option<Json<Option<HashMap<String, String>>>>,
    pub labels: Option<Json<Option<HashMap<String, String>>>>,
    pub status: DeploymentStatus,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub service: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEvent {
    pub id: Uuid,
    pub deployment_id: Uuid,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub event_type: String,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------
// HELPER STRUCTS FOR JSONB FIELDS
// ---------------------------------------------

/// Resource specification stored in the `resources` JSONB field
#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSpec {
    pub cpu_request_millicores: i32,
    pub cpu_limit_millicores: i32,
    pub memory_request_mb: i32,
    pub memory_limit_mb: i32,
}

impl Default for ResourceSpec {
    fn default() -> Self {
        Self {
            cpu_request_millicores: 250,
            cpu_limit_millicores: 500,
            memory_request_mb: 256,
            memory_limit_mb: 512,
        }
    }
}
