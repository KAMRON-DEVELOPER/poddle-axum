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
            vault_secret_path: d.vault_secret_path,
            secret_keys: d.secret_keys,
            environment_variables: d.environment_variables.and_then(|j| j.0).or_else(|| None),
            replicas: d.replicas,
            resources: d.resources.0,
            labels: d.labels.and_then(|j| j.0).or_else(|| None),
            status: d.status,
            cluster_namespace: d.cluster_namespace,
            cluster_deployment_name: d.cluster_deployment_name,
            subdomain: d.subdomain,
            custom_domain: d.custom_domain,
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
            vault_secret_path: d.vault_secret_path,
            secret_keys: d.secret_keys,
            environment_variables: d.environment_variables.and_then(|j| j.0).or_else(|| None),
            replicas: d.replicas,
            resources: d.resources.0,
            labels: d.labels.and_then(|j| j.0).or_else(|| None),
            status: d.status,
            cluster_namespace: d.cluster_namespace,
            cluster_deployment_name: d.cluster_deployment_name,
            subdomain: d.subdomain,
            custom_domain: d.custom_domain,
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
            replicas: req.replicas,
            port: req.port,
            environment_variables: req.environment_variables,
            secrets: req.secrets,
            resources: req.resources,
            labels: req.labels,
            subdomain: req.subdomain,
            custom_domain: req.custom_domain,
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
            replicas: req.replicas,
            port: req.port,
            environment_variables: req.environment_variables,
            secrets: req.secrets,
            resources: req.resources,
            labels: req.labels,
            subdomain: req.subdomain,
            custom_domain: req.custom_domain,
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
        writeln!(f, "  replicas: {}", self.replicas)?;
        writeln!(f, "  resources: {}", self.resources)?;

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

        if let Some(subdomain) = &self.subdomain {
            writeln!(f, "  subdomain: \"{}\"", subdomain)?;
        } else {
            writeln!(f, "  subdomain: None")?;
        }

        if let Some(custom_domain) = &self.custom_domain {
            writeln!(f, "  custom_domain: \"{}\"", custom_domain)?;
        } else {
            writeln!(f, "  custom_domain: None")?;
        }

        write!(f, "}}")
    }
}
