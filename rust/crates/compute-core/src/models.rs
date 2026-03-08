use std::collections::HashMap;

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type, types::Json};
use uuid::Uuid;

use crate::schemas::DeploymentSource;

// ---------------------------------------------
// ENUMS
// ---------------------------------------------

#[derive(Type, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, JsonSchema, Debug)]
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
            Self::Building => write!(f, "Building"),
            Self::Queued => write!(f, "Queued"),
            Self::Provisioning => write!(f, "Provisioning"),
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Unhealthy => write!(f, "Unhealthy"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Updating => write!(f, "Updating"),
            Self::Suspended => write!(f, "Suspended"),
            Self::Failed => write!(f, "Failed"),
            Self::BuildFailed => write!(f, "BuildFailed"),
            Self::Deleted => write!(f, "Deleted"),
            Self::ImagePullError => write!(f, "ImagePullError"),
        }
    }
}

#[derive(Type, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "deployment_event_type", rename_all = "snake_case")]
pub enum DeploymentEventType {
    StatusChanged,
    BuildStarted,
    BuildSucceeded,
    BuildFailed,
    DeploymentCreated,
    DeploymentUpdated,
    DeploymentDeleted,
    UnhealthyDetected,
    ImagePullFailed,
    SystemMessage,
}

impl std::fmt::Display for DeploymentEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StatusChanged => write!(f, "StatusChanged"),
            Self::BuildStarted => write!(f, "BuildStarted"),
            Self::BuildSucceeded => write!(f, "BuildSucceeded"),
            Self::BuildFailed => write!(f, "BuildFailed"),
            Self::DeploymentCreated => write!(f, "DeploymentCreated"),
            Self::DeploymentUpdated => write!(f, "DeploymentUpdated"),
            Self::DeploymentDeleted => write!(f, "DeploymentDeleted"),
            Self::UnhealthyDetected => write!(f, "UnhealthyDetected"),
            Self::ImagePullFailed => write!(f, "ImagePullFailed"),
            Self::SystemMessage => write!(f, "SystemMessage"),
        }
    }
}

#[derive(Type, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "deployment_event_level", rename_all = "snake_case")]
pub enum DeploymentEventLevel {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for DeploymentEventLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "Info"),
            Self::Warning => write!(f, "Warning"),
            Self::Error => write!(f, "Error"),
        }
    }
}

// ---------------------------------------------
// MODELS
// ---------------------------------------------

#[derive(FromRow, Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetRow {
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

#[derive(FromRow, Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddonPricesRow {
    pub id: Uuid,
    pub cpu_monthly_unit_price: BigDecimal,
    pub cpu_hourly_unit_price: BigDecimal,
    pub memory_monthly_unit_price: BigDecimal,
    pub memory_hourly_unit_price: BigDecimal,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub preset_id: Uuid,
    pub name: String,
    #[schemars(with = "DeploymentSource")]
    pub source: Json<DeploymentSource>,
    pub port: i32,
    pub desired_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub addon_cpu_millicores: Option<i32>,
    pub addon_memory_mb: Option<i32>,
    pub vault_secret_path: Option<String>,
    pub secret_keys: Option<Vec<String>>,
    #[schemars(with = "Option<HashMap<String, String>>")]
    pub environment_variables: Option<Json<Option<HashMap<String, String>>>>,
    #[schemars(with = "Option<HashMap<String, String>>")]
    pub labels: Option<Json<Option<HashMap<String, String>>>>,
    pub status: DeploymentStatus,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub service: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEventRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub event_type: DeploymentEventType,
    pub level: DeploymentEventLevel,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallationRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub installation_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------
// HELPER STRUCTS FOR JSONB FIELDS
// ---------------------------------------------

/// Resource specification stored in the `resources` JSONB field
#[derive(FromRow, Serialize, Deserialize, Clone, JsonSchema, Debug)]
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
