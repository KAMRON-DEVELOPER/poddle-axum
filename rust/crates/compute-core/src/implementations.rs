use std::fmt::Display;

use redis::{ErrorKind, FromRedisValue, RedisError, RedisResult, RedisWrite, ToRedisArgs, Value};
use uuid::Uuid;

use crate::{
    event::ComputeEvent,
    models::{Deployment, Preset, ResourceSpec},
    schemas::{
        CreateDeploymentMessage, CreateDeploymentRequest, DeploymentResponse, DeploymentsResponse,
        MetricSnapshot, PodMeta, PodPhase, UpdateDeploymentMessage, UpdateDeploymentRequest,
    },
};

impl<'a> ToRedisArgs for ComputeEvent<'a> {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        // to_vec is slightly more efficient than to_string
        let bytes = serde_json::to_vec(self).expect("ComputeEvent must serialize");
        out.write_arg(&bytes);
    }
}

impl From<&str> for PodPhase {
    fn from(value: &str) -> Self {
        match value {
            "Pending" => PodPhase::Pending,
            "Running" => PodPhase::Running,
            "Succeeded" => PodPhase::Succeeded,
            "Failed" => PodPhase::Failed,
            _ => PodPhase::Unknown,
        }
    }
}

impl Default for PodPhase {
    fn default() -> Self {
        Self::Unknown
    }
}

impl Display for PodPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PodPhase::Pending => "Pending",
            PodPhase::Running => "Running",
            PodPhase::Succeeded => "Succeeded",
            PodPhase::Failed => "Failed",
            PodPhase::Unknown => "Unknown",
        };
        f.write_str(s)
    }
}

impl PodMeta {
    pub fn as_redis_items(&self) -> Vec<(&'static str, String)> {
        vec![
            ("uid", self.uid.clone()),
            ("name", self.name.clone()),
            ("phase", self.phase.to_string()),
            ("restart_count", self.restart_count.to_string()),
        ]
    }
}

impl ToRedisArgs for MetricSnapshot {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        let bytes = serde_json::to_vec(self).expect("Failed to serialize MetricSnapshot");
        out.write_arg(&bytes);
    }
}

impl FromRedisValue for MetricSnapshot {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match v {
            // Redis returns data from Lists/Gets as BulkString (Vec<u8>)
            Value::BulkString(bytes) => serde_json::from_slice(bytes).map_err(|e| {
                RedisError::from((
                    ErrorKind::TypeError,
                    "Failed to deserialize MetricSnapshot from JSON",
                    e.to_string(),
                ))
            }),
            // Fallback: sometimes Redis returns simple strings for small ASCII values
            Value::SimpleString(s) => serde_json::from_str(s).map_err(|e| {
                RedisError::from((
                    ErrorKind::TypeError,
                    "Failed to deserialize MetricSnapshot from JSON",
                    e.to_string(),
                ))
            }),
            _ => Err(RedisError::from((
                ErrorKind::TypeError,
                "Expected BulkString (binary JSON) for MetricSnapshot",
            ))),
        }
    }
}

impl From<(Deployment, Vec<MetricSnapshot>)> for DeploymentsResponse {
    fn from((d, metrics): (Deployment, Vec<MetricSnapshot>)) -> Self {
        Self {
            id: d.id,
            user_id: d.user_id,
            project_id: d.project_id,
            name: d.name,
            image: d.image,
            port: d.port,
            desired_replicas: d.desired_replicas,
            ready_replicas: d.ready_replicas,
            available_replicas: d.available_replicas,
            preset_id: d.preset_id,
            addon_cpu_millicores: d.addon_cpu_millicores,
            addon_memory_mb: d.addon_memory_mb,
            vault_secret_path: d.vault_secret_path,
            secret_keys: d.secret_keys,
            environment_variables: d.environment_variables.and_then(|j| j.0).or_else(|| None),
            labels: d.labels.and_then(|j| j.0).or_else(|| None),
            status: d.status,
            domain: d.domain,
            subdomain: d.subdomain,
            created_at: d.created_at,
            updated_at: d.updated_at,
            metrics,
        }
    }
}

impl From<Deployment> for DeploymentResponse {
    fn from(d: Deployment) -> Self {
        Self {
            id: d.id,
            user_id: d.user_id,
            project_id: d.project_id,
            name: d.name,
            image: d.image,
            port: d.port,
            desired_replicas: d.desired_replicas,
            ready_replicas: d.ready_replicas,
            available_replicas: d.available_replicas,
            preset_id: d.preset_id,
            addon_cpu_millicores: d.addon_cpu_millicores,
            addon_memory_mb: d.addon_memory_mb,
            vault_secret_path: d.vault_secret_path,
            secret_keys: d.secret_keys,
            environment_variables: d.environment_variables.and_then(|j| j.0).or_else(|| None),
            labels: d.labels.and_then(|j| j.0).or_else(|| None),
            status: d.status,
            domain: d.domain,
            subdomain: d.subdomain,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

impl From<(Uuid, Uuid, Uuid, Preset, CreateDeploymentRequest)> for CreateDeploymentMessage {
    fn from(
        (user_id, project_id, deployment_id, preset, req): (
            Uuid,
            Uuid,
            Uuid,
            Preset,
            CreateDeploymentRequest,
        ),
    ) -> Self {
        let resource_spec = ResourceSpec {
            cpu_request_millicores: preset.cpu_millicores
                + req.addon_cpu_millicores.unwrap_or_default(),
            cpu_limit_millicores: preset.cpu_millicores
                + req.addon_cpu_millicores.unwrap_or_default(),
            memory_request_mb: preset.memory_mb + req.addon_memory_mb.unwrap_or_default(),
            memory_limit_mb: preset.memory_mb + req.addon_memory_mb.unwrap_or_default(),
        };

        Self {
            user_id,
            project_id,
            deployment_id,
            name: req.name,
            image: req.image,
            image_pull_secret: req.image_pull_secret,
            port: req.port,
            desired_replicas: req.desired_replicas,
            preset_id: req.preset_id,
            resource_spec: resource_spec,
            environment_variables: req.environment_variables,
            secrets: req.secrets,
            labels: req.labels,
            domain: req.domain,
            subdomain: req.subdomain,
        }
    }
}

impl From<(Uuid, Uuid, Uuid, Option<Preset>, UpdateDeploymentRequest)> for UpdateDeploymentMessage {
    fn from(
        (user_id, project_id, deployment_id, preset, req): (
            Uuid,
            Uuid,
            Uuid,
            Option<Preset>,
            UpdateDeploymentRequest,
        ),
    ) -> Self {
        let resource_spec = preset.map(|preset| ResourceSpec {
            cpu_request_millicores: preset.cpu_millicores
                + req.addon_cpu_millicores.unwrap_or_default(),
            cpu_limit_millicores: preset.cpu_millicores
                + req.addon_cpu_millicores.unwrap_or_default(),
            memory_request_mb: preset.memory_mb + req.addon_memory_mb.unwrap_or_default(),
            memory_limit_mb: preset.memory_mb + req.addon_memory_mb.unwrap_or_default(),
        });

        Self {
            user_id,
            project_id,
            deployment_id,
            name: req.name,
            image: req.image,
            image_pull_secret: req.image_pull_secret,
            port: req.port,
            desired_replicas: req.desired_replicas,
            preset_id: req.preset_id,
            resource_spec: resource_spec,
            environment_variables: req.environment_variables,
            secrets: req.secrets,
            labels: req.labels,
            domain: req.domain,
            subdomain: req.subdomain,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
