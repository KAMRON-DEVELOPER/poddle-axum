use kube::Client as KubeClient;
use prometheus_http_query::Client as PrometheusClient;
use redis::aio::MultiplexedConnection;
use shared::utilities::errors::AppError;

pub async fn metrics_scraper(
    client: KubeClient,
    prometheus: PrometheusClient,
    connection: MultiplexedConnection,
) -> Result<(), AppError> {
    Ok(())
}
