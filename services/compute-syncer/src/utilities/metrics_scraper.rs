use chrono::Utc;
use prometheus_http_query::Client as PrometheusClient;
use shared::schemas::{DeploymentMetrics, MetricPoint};
use shared::services::redis::Redis;
use shared::utilities::cache_keys::CacheKeys;
use shared::utilities::channel_names::ChannelNames;
use shared::utilities::config::Config;
use shared::utilities::errors::AppError;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info};

#[derive(Default)]
struct AggregatedValue {
    deployment_id: String,
    cpu: f64,
    memory: f64,
}

pub async fn metrics_scraper(
    config: Config,
    prometheus: PrometheusClient,
    redis: Redis,
) -> Result<(), AppError> {
    info!("📈 Starting Prometheus metrics scraper");
    info!(
        "⚙️  Scrape interval: {}s, History points: {}",
        config.scrape_interval_seconds, config.history_points_to_keep
    );

    let mut interval = tokio::time::interval(Duration::from_secs(config.scrape_interval_seconds));

    loop {
        interval.tick().await;

        if let Err(e) = scrape(&config, &prometheus, redis.clone()).await {
            error!("Failed to scrape metrics: {}", e);
        }
    }
}

async fn scrape(
    config: &Config,
    prometheus: &PrometheusClient,
    mut redis: Redis,
) -> Result<(), AppError> {
    // labels are stored in kube-state-metrics and it exports a specific metric called kube_pod_labels
    // JOIN kube_pod_labels with container metrics
    // container_cpu_usage_seconds_total comes from cAdvisor (embedded in Kubelet).
    // It knows about low-level details like pod, namespace, and image, but it is unaware of your high-level Kubernetes labels
    let cpu_query = r#"
        sum(
            rate(
                container_cpu_usage_seconds_total{
                    container!="",
                    container!="POD",
                    namespace=~"user-.*"
                }[5m]
            ) 
            * on(pod, namespace) group_left(label_deployment_id)
            kube_pod_labels{label_managed_by="poddle"}
        ) by (pod, namespace, label_deployment_id, label_project_id)
    "#;

    let memory_query = r#"
        sum(
            container_memory_working_set_bytes{
                container!="",
                container!="POD",
                namespace=~"user-.*"
            }
            * on(pod, namespace) group_left(label_deployment_id)
            kube_pod_labels{label_managed_by="poddle"}
        ) by (pod, namespace, label_deployment_id, label_project_id)
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

    // info!("cpu_result: {:?}", cpu_result);
    // info!("memory_result: {:?}", memory_result);

    // Aggregation
    let mut project_aggregates: HashMap<String, Vec<AggregatedValue>> = HashMap::new();
    let now = Utc::now().timestamp();

    // Parse CPU
    if let prometheus_http_query::response::Data::Vector(cpu_vector) = cpu_result.data() {
        for sample in cpu_vector {
            if let Some(project_id) = sample.metric().get("label_project_id") {
                let val = sample.sample().value() * 1000.0;
                project_aggregates
                    .entry(project_id)
                    .or_default()
                    .push(AggregatedValue {
                        deployment_id,
                        cpu,
                        memory,
                    });
            }
        }
    }

    // Parse Memory
    if let prometheus_http_query::response::Data::Vector(memory_vector) = memory_result.data() {
        for sample in memory_vector {
            if let Some(project_id) = sample.metric().get("label_project_id") {
                let val = sample.sample().value();
                aggregates.entry(project_id.clone()).or_default().memory += val;
            }
        }
    }

    // Pipeline to Redis
    let mut pipe = redis::pipe();
    let history_limit = config.history_points_to_keep as i64;

    for (project_id, deployments) in project_aggregates {
        let channel = ChannelNames::project_metrics(&project_id);

        let payload = serde_json::json!({
            "type": "metrics_update",
            "timestamp": now,
            "deployments": deployments.iter().map(|d| {
                json!({
                    "id": d.deployment_id,
                    "cpu": d.cpu,
                    "memory": d.memory
                })
            }).collect::<Vec<_>>()
        });

        pipe.publish(channel, payload.to_string());
    }

    for (deployment_id, values) in &aggregates {
        let key = CacheKeys::deployment_metrics(deployment_id);
        let pubsub_channel = ChannelNames::deployment_metrics(deployment_id);

        let cpu_point = MetricPoint {
            ts: now,
            v: values.cpu,
        };
        let memory_point = MetricPoint {
            ts: now,
            v: values.memory,
        };

        // Initialize key if missing
        // If key doesn't exist, create empty structure
        let initial = DeploymentMetrics {
            cpu_history: vec![],
            memory_history: vec![],
        };
        pipe.cmd("JSON.SET")
            .arg(&key)
            .arg("$")
            .arg(&initial)
            .arg("NX");

        // Append new points to history arrays
        let _ = pipe.json_arr_append(&key, "$.cpuHistory", &cpu_point);
        let _ = pipe.json_arr_append(&key, "$.memoryHistory", &memory_point);

        let _ = pipe.json_arr_trim(&key, "$.cpuHistory", -history_limit, -1);
        let _ = pipe.json_arr_trim(&key, "$.memoryHistory", -history_limit, -1);

        let message = serde_json::json!({
            "cpuHistory": cpu_point,
            "memoryHistory": memory_point
        });
        pipe.publish(pubsub_channel, message.to_string());

        let ttl = config.scrape_interval_seconds * config.history_points_to_keep;
        pipe.expire(&key, ttl.try_into().unwrap());
    }

    // Execute updates in ONE network round-trip
    let _: () = pipe.query_async(&mut redis.connection).await?;

    info!(
        "✅ Aggregated and cached metrics for {} deployments",
        aggregates.len()
    );

    Ok(())
}
