use http_contracts::list::schema::ListResponse;
use uuid::Uuid;

use crate::{
    models::{Deployment, Preset, ResourceSpec},
    schemas::{
        CreateDeploymentMessage, CreateDeploymentRequest, DeploymentResponse, DeploymentsResponse,
        MetricHistory, Pod, PodPhase, UpdateDeploymentMessage, UpdateDeploymentRequest,
    },
};

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

impl From<(Deployment, MetricHistory)> for DeploymentsResponse {
    fn from((d, dm): (Deployment, MetricHistory)) -> Self {
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
            metrics: dm.snapshots,
        }
    }
}

impl From<(Deployment, ListResponse<Pod>)> for DeploymentResponse {
    fn from((d, pods): (Deployment, ListResponse<Pod>)) -> Self {
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
            pods,
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
