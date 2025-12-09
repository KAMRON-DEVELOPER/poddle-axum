use uuid::Uuid;

use crate::{
    models::Deployment,
    schemas::{
        CreateDeploymentMessage, CreateDeploymentRequest, DeploymentMetrics, DeploymentResponse,
        MessageResponse, Pagination, UpdateDeploymentMessage, UpdateDeploymentRequest,
    },
    utilities::errors::AppError,
};

impl MessageResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Pagination {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.offset < 0 {
            return Err(AppError::ValidationError(
                "Offset must be positive".to_string(),
            ));
        }

        if self.limit < 0 {
            return Err(AppError::ValidationError("Limit must positive".to_string()));
        } else if self.limit == 0 {
            return Err(AppError::ValidationError(
                "Limit must not be zero!".to_string(),
            ));
        }

        if self.limit > 100 {
            return Err(AppError::ValidationError(
                "Limit cannot exceed 100".to_string(),
            ));
        }

        Ok(())
    }
}

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
