use chrono::Utc;
use compute_core::{
    cache_keys::CacheKeys,
    channel_names::ChannelNames,
    schemas::{DeploymentMetrics, MetricSnapshot},
};
use factory::factories::redis::Redis;
use prometheus_http_query::Client as PrometheusClient;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info};

use crate::{config::Config, error::AppError};

pub async fn start_metrics_scraper(
    config: Config,
    prometheus: PrometheusClient,
    redis: Redis,
) -> Result<(), AppError> {
    info!("📈 Starting Prometheus metrics scraper");
    info!(
        "⚙️  Scrape interval: {}s, Snapshot to keep: {}",
        config.scrape_interval_seconds, config.metric_snapshots_to_keep
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
                container_cpu_usage_seconds_total{container!="",container!="POD",namespace=~"user-.*"}[5m]
            )
            * on(pod, namespace) group_left(label_deployment_id, label_project_id)
            kube_pod_labels{label_managed_by="poddle"}
        ) by (pod, namespace, label_deployment_id, label_project_id)
    "#;

    let memory_query = r#"
        sum(
            container_memory_working_set_bytes{container!="",container!="POD",namespace=~"user-.*"}
            * on(pod, namespace) group_left(label_deployment_id, label_project_id)
            kube_pod_labels{label_managed_by="poddle"}
        ) by (pod, namespace, label_deployment_id, label_project_id)
    "#;

    // Execute queries
    let (cpu_result, memory_result) = tokio::try_join!(
        prometheus.query(cpu_query).get(),
        prometheus.query(memory_query).get()
    )
    .map_err(|e| AppError::InternalError(format!("Prometheus query failed: {}", e)))?;

    // info!("cpu_result: {:?}", cpu_result);
    // info!("memory_result: {:?}", memory_result);

    // Aggregate Data
    // Structure: ProjectID -> DeploymentID -> Values
    let mut project_map: HashMap<String, HashMap<String, MetricSnapshot>> = HashMap::new();
    let now = Utc::now().timestamp();

    // Helper closure to process vector results
    let mut process_vector = |data: &prometheus_http_query::response::Data, is_cpu: bool| {
        if let prometheus_http_query::response::Data::Vector(vec) = data {
            for instant_vector in vec {
                let metric = instant_vector.metric();

                // Safely extract labels
                if let (Some(project_id), Some(deployment_id)) = (
                    metric.get("label_project_id"),
                    metric.get("label_deployment_id"),
                ) {
                    let value = instant_vector.sample().value();

                    let deployment_metric_point_entry = project_map
                        .entry(project_id.clone())
                        .or_default()
                        .entry(deployment_id.clone())
                        .or_default();

                    if is_cpu {
                        // CPU is usually in cores, multiply by 1000 for millicores
                        deployment_metric_point_entry.cpu += value * 1000.0;
                    } else {
                        // Memory is in bytes, convert to MB if needed, or keep bytes.
                        // Frontend usually expects MB. Here we keep raw bytes or convert:
                        deployment_metric_point_entry.memory += value / 1024.0 / 1024.0;
                    }
                }
            }
        }
    };

    process_vector(cpu_result.data(), true);
    process_vector(memory_result.data(), false);

    // Pipeline to Redis
    let mut pipe = redis::pipe();
    let metric_snapshots_to_keep = config.metric_snapshots_to_keep as i64;
    let mut total_deployments = 0;

    for (project_id, deployment_map) in project_map {
        let mut project_payloads = Vec::new();

        for (deployment_id, aggregated_value) in deployment_map {
            total_deployments += 1;

            // Add to Project Payload
            project_payloads.push(json!({
                "id": deployment_id,
                "cpu": aggregated_value.cpu,
                "memory": aggregated_value.memory
            }));

            let key = CacheKeys::deployment_metrics(&deployment_id);

            // Ensure key exists
            let initial = DeploymentMetrics { history: vec![] };
            pipe.cmd("JSON.SET")
                .arg(&key)
                .arg("$")
                .arg(&initial)
                .arg("NX")
                .ignore();

            let metric_snapshot = MetricSnapshot {
                ts: now,
                ..aggregated_value
            };
            let _ = pipe.json_arr_append(&key, "$.history", &metric_snapshot);
            let _ = pipe.json_arr_trim(&key, "$.history", -metric_snapshots_to_keep, -1);
            let ttl = config.scrape_interval_seconds * config.metric_snapshots_to_keep;
            pipe.expire(&key, ttl.try_into().unwrap()).ignore();
        }

        // Publish Project Batch Message
        if !project_payloads.is_empty() {
            let channel = ChannelNames::project_metrics(&project_id);
            let message = json!({
                "type": "metrics_update",
                "timestamp": now,
                "deployments": project_payloads
            });
            pipe.publish(channel, message.to_string()).ignore();
        }
    }

    // Execute Pipeline
    if total_deployments > 0 {
        let _: () = pipe
            .query_async(&mut redis.connection)
            .await
            .map_err(|e| AppError::InternalError(format!("Redis pipeline failed: {}", e)))?;

        info!("✅ Updated metrics for {} deployments", total_deployments);
    }

    Ok(())
}
