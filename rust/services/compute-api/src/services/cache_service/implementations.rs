use compute_core::{
    cache_keys::CacheKeys,
    schemas::{MetricHistory, MetricSnapshot, Pod, PodHistory},
};
use http_contracts::pagination::schema::Pagination;
use redis::{AsyncTypedCommands, aio::MultiplexedConnection};
use tracing::{error, info};

use crate::{error::AppError, services::cache_service::CacheService};

impl CacheService {
    /// Get pods with metrics for a deployment (Deployment Page)
    #[tracing::instrument(name = "cache_service.get_deployment_pods", skip_all, err)]
    pub async fn get_deployment_pods(
        id: &str,
        count: isize,
        p: &Pagination,
        con: &mut MultiplexedConnection,
    ) -> Result<Vec<Pod>, AppError> {
        let index_key = CacheKeys::deployment_pods(&id);

        let start = p.offset as isize;
        let stop = (p.offset + p.limit) as isize - 1;

        // Get pod UIDs
        let uids = con.zrevrange(index_key, start, stop).await.map_err(|e| {
            error!(error = %e, "❌ Failed to get pod UIDs");
            AppError::InternalServerError(format!("❌ Failed to get pod UIDs: {}", e))
        })?;

        if uids.is_empty() {
            return Ok(Vec::new());
        }

        let mut p = redis::pipe();

        for uid in &uids {
            let meta_key = CacheKeys::deployment_pod_meta(id, uid);
            let metrics_key = CacheKeys::deployment_pod_metrics(id, uid);

            p.hgetall(meta_key); // Metadata (Hash -> Struct)
            p.lrange(metrics_key, 0, count - 1); // Metrics (List -> Vec<Struct>)
        }

        let start = std::time::Instant::now();

        // Execute Pipeline
        // The power of redis-rs: It deserializes the flat stream into tuples!
        // Expect: Vec<(PodHistory, Vec<MetricSnapshot>)>
        let results: Vec<(PodHistory, Vec<MetricSnapshot>)> =
            p.query_async(con).await.map_err(|e| {
                error!(error = %e, "❌ Redis pipeline failed");
                AppError::InternalServerError(format!("❌ Redis pipeline failed: {}", e))
            })?;

        info!(
            elapsed = start.elapsed().as_millis(),
            pods_count = uids.len(),
            "🏁 Pod metrics fetched"
        );

        let pods: Vec<Pod> = results
            .into_iter()
            .map(|(meta, metrics)| Pod { meta, metrics })
            .collect();

        Ok(pods)
    }

    /// Get aggregated metrics for multiple deployments (Project Page)
    #[tracing::instrument(name = "cache_service.get_deployments_metrics", skip_all, err)]
    pub async fn get_deployments_metrics(
        ids: Vec<&str>,
        count: isize,
        con: &mut MultiplexedConnection,
    ) -> Result<Vec<MetricHistory>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut p = redis::pipe();

        for id in &ids {
            let key = CacheKeys::deployment_metrics(&id);
            p.lrange(key, 0, -count - 1);
        }

        let start = std::time::Instant::now();

        let results: Vec<Vec<MetricSnapshot>> = p.query_async(con).await.map_err(|e| {
            error!(error = %e, "❌ Redis pipeline failed");
            AppError::InternalServerError(format!("❌ Redis pipeline failed: {}", e))
        })?;

        info!(
            elapsed = start.elapsed().as_millis(),
            "🏁 Deployment metrics fetched"
        );

        let metrics = results
            .into_iter()
            .map(|snapshots| MetricHistory { snapshots })
            .collect();

        Ok(metrics)
    }
}
