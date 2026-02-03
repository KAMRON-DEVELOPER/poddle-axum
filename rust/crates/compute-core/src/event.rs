use redis_macros::ToRedisArgs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{models::DeploymentStatus, schemas::MetricSnapshot};

#[derive(ToRedisArgs, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ComputeEvent<'a> {
    // Corresponds to: { "type": "MetricsUpdate", "snapshot": { ts: ..., cpu: ..., memory: ... }, ... }
    MetricsUpdate {
        id: String,
        snapshot: MetricSnapshot,
    },
    // Corresponds to: { "type": "StatusUpdate", "deployment_id": "...", "status": "...", ... }
    StatusUpdate {
        id: &'a Uuid,
        status: DeploymentStatus,
    },
    // Corresponds to: { "type": "SystemMessage", "deployment_id": "...", "message": "...", "level": "...", ... }
    SystemMessage {
        deployment_id: &'a Uuid,
        message: String,
        level: EventLevel,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EventLevel {
    Info,
    Error,
    Warning,
    Success,
}
