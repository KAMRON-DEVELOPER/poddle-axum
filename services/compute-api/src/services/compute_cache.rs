use redis::{AsyncCommands, aio::MultiplexedConnection};
use shared::{
    schemas::{DeploymentMetrics, DeploymentResponse},
    utilities::errors::AppError,
};
use uuid::Uuid;

pub struct ComputeCache(pub MultiplexedConnection);

impl ComputeCache {
    pub async fn get_deployments_state(
        &mut self,
        project_id: Uuid,
    ) -> Result<Vec<DeploymentMetrics>, AppError> {
        let state: Vec<DeploymentMetrics> = self.0.get("aaa").await?;

        Ok(state)
    }

    pub async fn get_deployment_state(
        &mut self,
        deployment_id: Uuid,
    ) -> Result<DeploymentMetrics, AppError> {
        let state: DeploymentMetrics = self.0.get("aaa").await?;

        Ok(state)
    }

    pub async fn set_deployment_state(
        &mut self,
        id: Uuid,
        state: DeploymentMetrics,
    ) -> Result<(), AppError> {
        let _: () = self.0.set("user_async", &state).await?;
        Ok(())
    }
}
