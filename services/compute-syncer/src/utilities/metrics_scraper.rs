use kube::Client as KubeClient;
use prometheus_http_query::Client as PrometheusClient;
use redis::aio::MultiplexedConnection;
use shared::schemas::{DeploymentMetrics, PodMetrics, PodPhase};
use shared::utilities::errors::AppError;
use std::time::Duration;
use tracing::{error, info};

pub async fn metrics_scraper(
    _client: KubeClient,
    prometheus: PrometheusClient,
    mut connection: MultiplexedConnection,
) -> Result<(), AppError> {
    info!("📈 Starting Prometheus metrics scraper");

    let mut interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        interval.tick().await;

        if let Err(e) = scrape_and_cache_metrics(&prometheus, &mut connection).await {
            error!("Failed to scrape metrics: {}", e);
        }
    }
}

async fn scrape_and_cache_metrics(
    prometheus: &PrometheusClient,
    connection: &mut MultiplexedConnection,
) -> Result<(), AppError> {
    // Query CPU usage per pod
    let cpu_query = r#"
        sum(rate(container_cpu_usage_seconds_total{
            namespace=~"user-.*",
            container!="",
            container!="POD"
        }[5m])) by (pod, namespace)
    "#;

    // Query memory usage per pod
    let memory_query = r#"
        sum(container_memory_working_set_bytes{
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

// Helper function to retrieve cached metrics for API responses
pub async fn get_deployment_metrics(
    connection: &mut MultiplexedConnection,
    deployment_id: &str,
    pod_names: Vec<String>,
) -> Result<DeploymentMetrics, AppError> {
    let cpu_key = format!("metrics:deployment:{}:cpu_total", deployment_id);
    let _cpu_total: Option<f64> = redis::cmd("GET")
        .arg(&cpu_key)
        .query_async(connection)
        .await?;

    let mut pod_metrics = Vec::new();

    for pod_name in pod_names {
        let cpu_key = format!("metrics:pod:{}:cpu", pod_name);
        let mem_key = format!("metrics:pod:{}:memory", pod_name);

        let cpu: Option<f64> = redis::cmd("GET")
            .arg(&cpu_key)
            .query_async(connection)
            .await?;
        let memory: Option<u64> = redis::cmd("GET")
            .arg(&mem_key)
            .query_async(connection)
            .await?;

        pod_metrics.push(PodMetrics {
            name: pod_name.clone(),
            phase: PodPhase::Running, // This should come from watcher cache
            cpu_millicores: cpu.unwrap_or(0.0),
            memory_bytes: memory.unwrap_or(0),
            restarts: 0,      // From watcher
            started_at: None, // From watcher
        });
    }

    Ok(DeploymentMetrics {
        deployment_id: deployment_id.to_string(),
        status: shared::models::DeploymentStatus::Healthy, // From DB
        replicas: pod_metrics.len() as i32,
        ready_replicas: pod_metrics.len() as i32,
        available_replicas: pod_metrics.len() as i32,
        unavailable_replicas: 0,
        pods: pod_metrics,
        timestamp: chrono::Utc::now().timestamp(),
    })
}
