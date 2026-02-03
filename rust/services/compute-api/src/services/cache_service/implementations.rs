use compute_core::{
    cache_keys::CacheKeys,
    schemas::{MetricHistory, MetricSnapshot, Pod},
};
use http_contracts::pagination::schema::Pagination;
use redis::{AsyncTypedCommands as _, JsonAsyncCommands, aio::MultiplexedConnection, pipe};
use tracing::{error, info};

use crate::{error::AppError, services::cache_service::CacheService};

impl CacheService {
    /// Produce deployment inside pod level metrics
    #[tracing::instrument(name = "cache_service.get_deployment_pods", skip_all, err)]
    pub async fn get_deployment_pods(
        id: &str,
        points_count: u64,
        p: &Pagination,
        con: &mut MultiplexedConnection,
    ) -> Result<Vec<Pod>, AppError> {
        let key = CacheKeys::deployment_pod_names(&id);
        let start = p.limit as isize;
        let stop = p.offset as isize;

        let names = con.zrevrange(key, start, stop).await?;

        if names.is_empty() {
            return Ok(Vec::new());
        }

        let keys = CacheKeys::deployment_pods_metrics(id, &names);
        let path = format!("$.snapshots[-{}:]", points_count);

        let query_start = std::time::Instant::now();
        let results: Vec<Option<Vec<String>>> = con.json_get(&keys, &path).await.map_err(|e| {
            error!(error = %e, "❌ Redis query failed");
            AppError::InternalServerError(format!("❌ Redis query failed: {}", e))
        })?;
        info!(
            query_elapsed = query_start.elapsed().as_millis(),
            "🏁 Pipeline query completed"
        );

        let mget_results = results.into_iter().next().flatten().unwrap_or_default();

        let pods = names
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                let snapshots = mget_results
                    .get(i)
                    .and_then(|s| serde_json::from_str::<Vec<MetricSnapshot>>(s).ok())
                    .unwrap_or_default();

                Pod {
                    name,
                    metrics: snapshots,
                }
            })
            .collect::<Vec<Pod>>();

        Ok(pods)
    }

    #[tracing::instrument(name = "cache_service.get_deployments_metrics", skip_all, err)]
    pub async fn get_deployments_metrics(
        points_count: u64,
        ids: Vec<&str>,
        con: &mut MultiplexedConnection,
    ) -> Result<Vec<MetricHistory>, AppError> {
        // Safety check for Redis syntax
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let keys = CacheKeys::deployments_metrics(&ids);
        let path = format!("$.snapshots[-{}:]", points_count);

        let pipeline_start = std::time::Instant::now();
        let mut p: redis::Pipeline = pipe();
        p.json_get(&keys, &path)?;

        let results: Vec<Option<Vec<String>>> = p.query_async(con).await.map_err(|e| {
            error!(error = %e, "❌ Redis pipeline failed");
            AppError::InternalServerError(format!("❌ Redis pipeline failed: {}", e))
        })?;
        info!(
            pipeline_elapsed = pipeline_start.elapsed().as_millis(),
            "🏁 Pipeline query completed"
        );

        // Extract MGET Results safely
        // Take the first result, flatten the Option, and default to empty Vec on failure
        let mget_results = results.into_iter().next().flatten().unwrap_or_default();

        // Map to Domain Objects with Length Guarantee
        // We iterate over the INPUT length (ids), not the Redis result length.
        // This ensures that if Redis returns fewer items or fails, we pad with empty metrics
        // rather than dropping the deployment from the UI.
        let metrics = (0..ids.len())
            .map(|i| {
                // Try to get the JSON string at index `i`
                let snapshots = mget_results
                    .get(i)
                    // If found, try to parse it
                    .and_then(|s| serde_json::from_str::<Vec<MetricSnapshot>>(s).ok())
                    // If index missing OR parsing failed, return empty snapshots
                    .unwrap_or_default();

                MetricHistory { snapshots }
            })
            .collect();

        Ok(metrics)
    }
}
