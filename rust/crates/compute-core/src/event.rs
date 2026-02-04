use redis_macros::ToRedisArgs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    models::DeploymentStatus,
    schemas::{DeploymentMetricUpdate, Pod, PodMetricUpdate},
};

#[derive(ToRedisArgs, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ComputeEvent<'a> {
    DeploymentMetricsUpdate {
        updates: Vec<DeploymentMetricUpdate>,
    },

    DeploymentStatusUpdate {
        id: &'a Uuid,
        status: DeploymentStatus,
    },

    DeploymentSystemMessage {
        id: &'a Uuid,
        message: String,
        level: EventLevel,
    },

    PodMetricsUpdate {
        updates: Vec<PodMetricUpdate>,
    },

    PodApply {
        pod: Pod,
    },

    PodDelete {
        uid: String,
    },

    PodStatusUpdate {
        uid: &'a String,
        status: DeploymentStatus,
    },

    PodSystemMessage {
        uid: &'a String,
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
