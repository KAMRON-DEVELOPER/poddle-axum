use std::fmt;

use uuid::Uuid;

use crate::{
    models::{Deployment, ResourceSpec},
    schemas::{
        CreateDeploymentMessage, CreateDeploymentRequest, DeploymentMetrics, DeploymentResponse,
        UpdateDeploymentMessage, UpdateDeploymentRequest,
    },
};

impl DeploymentResponse {
    pub fn from_parts(d: Deployment, dm: DeploymentMetrics) -> Self {
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
            history: dm.history,
        }
    }
}

impl From<(Deployment, DeploymentMetrics)> for DeploymentResponse {
    fn from((d, dm): (Deployment, DeploymentMetrics)) -> Self {
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

            history: dm.history,
        }
    }
}

impl From<(Uuid, Uuid, Uuid, CreateDeploymentRequest)> for CreateDeploymentMessage {
    fn from(
        (user_id, project_id, deployment_id, req): (Uuid, Uuid, Uuid, CreateDeploymentRequest),
    ) -> Self {
        Self {
            user_id,
            project_id,
            deployment_id,
            name: req.name,
            image: req.image,
            desired_replicas: req.desired_replicas,
            port: req.port,
            preset_id: req.preset_id,
            addon_cpu_millicores: req.addon_cpu_millicores,
            addon_memory_mb: req.addon_memory_mb,
            environment_variables: req.environment_variables,
            secrets: req.secrets,
            labels: req.labels,
            domain: req.domain,
            subdomain: req.subdomain,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

impl From<(Uuid, Uuid, Uuid, UpdateDeploymentRequest)> for UpdateDeploymentMessage {
    fn from(
        (user_id, project_id, deployment_id, req): (Uuid, Uuid, Uuid, UpdateDeploymentRequest),
    ) -> Self {
        Self {
            user_id,
            project_id,
            deployment_id,
            name: req.name,
            image: req.image,
            desired_replicas: req.desired_replicas,
            port: req.port,
            preset_id: req.preset_id,
            addon_cpu_millicores: req.addon_cpu_millicores,
            addon_memory_mb: req.addon_memory_mb,
            environment_variables: req.environment_variables,
            secrets: req.secrets,
            labels: req.labels,
            domain: req.domain,
            subdomain: req.subdomain,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

impl fmt::Display for ResourceSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ResourceSpec {{ CPU: {}-{} millicores, Memory: {}-{} MB }}",
            self.cpu_request_millicores,
            self.cpu_limit_millicores,
            self.memory_request_mb,
            self.memory_limit_mb
        )
    }
}

impl fmt::Display for CreateDeploymentRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "CreateDeploymentRequest {{")?;
        writeln!(f, "  name: \"{}\"", self.name)?;
        writeln!(f, "  image: \"{}\"", self.image)?;
        writeln!(f, "  port: {}", self.port)?;
        writeln!(f, "  desired_replicas: {}", self.desired_replicas)?;
        writeln!(f, "  preset_id: {}", self.preset_id)?;
        writeln!(f, "  addon_cpu_millicores: {:?}", self.addon_cpu_millicores)?;
        writeln!(f, "  addon_memory_mb: {:?}", self.addon_memory_mb)?;

        if let Some(secrets) = &self.secrets {
            writeln!(f, "  secrets: {{")?;
            for (key, _) in secrets {
                writeln!(f, "    {}: [REDACTED]", key)?;
            }
            writeln!(f, "  }}")?;
        } else {
            writeln!(f, "  secrets: None")?;
        }

        if let Some(env_vars) = &self.environment_variables {
            writeln!(f, "  environment_variables: {{")?;
            for (key, value) in env_vars {
                writeln!(f, "    {}: \"{}\"", key, value)?;
            }
            writeln!(f, "  }}")?;
        } else {
            writeln!(f, "  environment_variables: None")?;
        }

        if let Some(labels) = &self.labels {
            writeln!(f, "  labels: {{")?;
            for (key, value) in labels {
                writeln!(f, "    {}: \"{}\"", key, value)?;
            }
            writeln!(f, "  }}")?;
        } else {
            writeln!(f, "  labels: None")?;
        }

        if let Some(domain) = &self.domain {
            writeln!(f, "  domain: \"{}\"", domain)?;
        } else {
            writeln!(f, "  domain: None")?;
        }

        if let Some(subdomain) = &self.subdomain {
            writeln!(f, "  subdomain: \"{}\"", subdomain)?;
        } else {
            writeln!(f, "  subdomain: None")?;
        }

        write!(f, "}}")
    }
}
