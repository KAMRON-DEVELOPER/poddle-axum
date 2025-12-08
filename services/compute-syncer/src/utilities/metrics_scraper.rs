use prometheus_http_query::Client as PrometheusClient;
use redis::aio::MultiplexedConnection;
use shared::utilities::errors::AppError;
use shared::utilities::{cache_keys::CacheKeys, config::Config};
use std::time::Duration;
use tracing::{error, info};

pub async fn metrics_scraper(
    config: Config,
    prometheus: PrometheusClient,
    mut connection: MultiplexedConnection,
) -> Result<(), AppError> {
    info!("📈 Starting Prometheus metrics scraper");
    info!(
        "⚙️  Scrape interval: {}s, History points: {}",
        config.scrape_interval_seconds, config.history_points_to_keep
    );

    let mut interval = tokio::time::interval(Duration::from_secs(config.scrape_interval_seconds));

    loop {
        interval.tick().await;

        if let Err(e) = scrape(&config, &prometheus, &mut connection).await {
            error!("Failed to scrape metrics: {}", e);
        }
    }
}

async fn scrape(
    config: &Config,
    prometheus: &PrometheusClient,
    connection: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    let cpu_query = r#"
        sum(rate(container_cpu_usage_seconds_total{
            namespace=~"user-.*",
            container!="",
            container!="POD"
        }[5m])) by (pod, namespace)
    "#;

    let memory_query = r#"
        sum(container_memory_working_set_bytes{
            namespace=~"user-.*",
            pod=~".*",
            container!="",
            container!="POD"
        }) by (pod, namespace)
    "#;

    // Execute queries
    let cpu_result = prometheus
        .query(cpu_query)
        .get()
        .await
        .map_err(|e| AppError::InternalError(format!("Prometheus CPU query failed: {}", e)))?;

    let memory_result =
        prometheus.query(memory_query).get().await.map_err(|e| {
            AppError::InternalError(format!("Prometheus memory query failed: {}", e))
        })?;

    let default_ns = "default".to_string();
    let ttl = config.scrape_interval_seconds * config.history_points_to_keep;
    // let cache_key = CacheKeys::pod_metrics(pod_id);

    // Process CPU metrics
    if let prometheus_http_query::response::Data::Vector(cpu_vector) = cpu_result.data() {
        for sample in cpu_vector {
            if let Some(pod_name) = sample.metric().get("pod") {
                let namespace = sample
                    .metric()
                    .get("namespace")
                    .unwrap_or_else(|| &default_ns);
                let cpu_millicores = sample.sample().value() * 1000.0; // Convert to millicores

                // Only cache metrics for our managed namespaces
                if !namespace.starts_with("user-") {
                    continue;
                }

                let cache_key = format!("metrics:pod:{}:{}:cpu", namespace, pod_name);
                let _: () = redis::cmd("SETEX")
                    .arg(&cache_key)
                    .arg(60) // 1 minute TTL
                    .arg(cpu_millicores)
                    .query_async(connection)
                    .await?;
            }
        }
    }

    // Process memory metrics
    if let prometheus_http_query::response::Data::Vector(memory_vector) = memory_result.data() {
        for sample in memory_vector {
            if let Some(pod_name) = sample.metric().get("pod") {
                let namespace = sample
                    .metric()
                    .get("namespace")
                    .unwrap_or_else(|| &default_ns);
                let memory_bytes = sample.sample().value() as u64;

                if !namespace.starts_with("user-") {
                    continue;
                }

                let cache_key = format!("metrics:pod:{}:{}:memory", namespace, pod_name);
                let _: () = redis::cmd("SETEX")
                    .arg(&cache_key)
                    .arg(60)
                    .arg(memory_bytes)
                    .query_async(connection)
                    .await?;
            }
        }
    }

    // Query and cache deployment-level aggregates
    let deployment_query = r#"
        sum(rate(container_cpu_usage_seconds_total{
            pod=~".*",
            container!="",
            container!="POD"
        }[5m])) by (label_deployment_id)
    "#;

    let deployment_result = prometheus
        .query(deployment_query)
        .get()
        .await
        .map_err(|e| AppError::InternalError(format!("Deployment query failed: {}", e)))?;

    if let prometheus_http_query::response::Data::Vector(dep_vector) = deployment_result.data() {
        for sample in dep_vector {
            if let Some(deployment_id) = sample.metric().get("label_deployment_id") {
                let total_cpu = sample.sample().value() * 1000.0;

                let cache_key = format!("metrics:deployment:{}:cpu_total", deployment_id);
                let _: () = redis::cmd("SETEX")
                    .arg(&cache_key)
                    .arg(60)
                    .arg(total_cpu)
                    .query_async(connection)
                    .await?;

                info!(
                    "📊 Cached metrics for deployment {}: {:.2} millicores",
                    deployment_id, total_cpu
                );
            }
        }
    }

    Ok(())
}
