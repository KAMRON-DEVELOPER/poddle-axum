use redis::{JsonAsyncCommands, aio::MultiplexedConnection};
use uuid::Uuid;
use compute_core::schemas::PodMetrics;
use crate::error::AppError;

pub struct ComputeCache(pub MultiplexedConnection);

impl ComputeCache {
    pub async fn get_pod_metrics(
        &mut self,
        deployment_id: Uuid,
    ) -> Result<Vec<PodMetrics>, AppError> {
        let key = format!("deployments:{}:metrics", deployment_id);

        // match self.0.json_get::<_, _, Vec<PodMetrics>>(key, "$").await {
        //     Ok(metrics) => Ok(metrics),
        //     Err(e) => Err(AppError::RedisError(e)),
        // }

        let metrics = self.0.json_get::<_, _, Vec<PodMetrics>>(key, "$").await?;
        Ok(metrics)
    }

    pub async fn set_pod_metrics(
        &mut self,
        deployment_id: Uuid,
        state: PodMetrics,
    ) -> Result<(), AppError> {
        let key = format!("deployments:{}:metrics", deployment_id);
        let _: () = self.0.json_set(key, "$", &state).await?;
        Ok(())
    }
}
