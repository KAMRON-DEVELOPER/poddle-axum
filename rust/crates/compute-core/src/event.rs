use redis_macros::ToRedisArgs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    models::DeploymentStatus,
    schemas::{DeploymentMetricUpdate, PodMetricUpdate},
};

#[derive(ToRedisArgs, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ComputeEvent<'a> {
    // Corresponds to: { "type": "DeploymentMetricsUpdate", "updates": { id: ..., snapshot: { ts: ..., cpu: ..., memory: ... } }, ... }
    DeploymentMetricsUpdate {
        updates: Vec<DeploymentMetricUpdate>,
    },
    // Corresponds to: { "type": "PodMetricsUpdate", "updates": { pod_name: ..., phase: ..., snapshot: { ts: ..., cpu: ..., memory: ... } }, ... }
    PodMetricsUpdate {
        updates: Vec<PodMetricUpdate>,
    },
    // Corresponds to: { "type": "DeploymentStatusUpdate", "deployment_id": "...", "status": "...", ... }
    DeploymentStatusUpdate {
        id: &'a Uuid,
        status: DeploymentStatus,
    },
    // Corresponds to: { "type": "DeploymentSystemMessage", "deployment_id": "...", "message": "...", "level": "...", ... }
    DeploymentSystemMessage {
        id: &'a Uuid,
        message: String,
        level: EventLevel,
    },

    LogAppend {
        deployment_id: &'a Uuid,
        cursor: String,
        timestamp_ns: i64,
        message: String,
        level: Option<String>,
        stream: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EventLevel {
    Info,
    Error,
    Warning,
    Success,
}
