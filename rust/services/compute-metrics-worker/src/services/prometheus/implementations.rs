use chrono::Utc;
use compute_core::{
    cache_keys::CacheKeys,
    channel_names::ChannelNames,
    configs::PrometheusConfig,
    event::ComputeEvent,
    schemas::{DeploymentMetricUpdate, DeploymentMetrics, MetricSnapshot},
};
use factory::factories::redis::Redis;
use prometheus_http_query::{Client, response::Data};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use crate::{error::AppError, services::prometheus::Prometheus};

pub async fn start_metrics_scraper(redis: Redis, prometheus: Prometheus) -> Result<(), AppError> {
    let client = prometheus.client;
    let cfg = prometheus.cfg;

    info!("📈 Starting Prometheus metrics scraper");
    info!(
        "⚙️  Scrape interval: {}s, Snapshot to keep: {}",
        cfg.scrape_interval_seconds, cfg.metric_snapshots_to_keep
    );

    let mut interval = tokio::time::interval(Duration::from_secs(cfg.scrape_interval_seconds));

    loop {
        interval.tick().await;

        if let Err(e) = scrape(&cfg, &client, redis.clone()).await {
            error!("Failed to scrape metrics: {}", e);
        }
    }
}

#[tracing::instrument(skip(cfg, client, redis), fields(scrape_id  = tracing::field::Empty))]
async fn scrape(cfg: &PrometheusConfig, client: &Client, mut redis: Redis) -> Result<(), AppError> {
    let start = std::time::Instant::now();
    let scrape_id = Uuid::new_v4();
    tracing::Span::current().record("scrape_id", &scrape_id.to_string());

    // labels are stored in kube-state-metrics and it exports a specific metric called kube_pod_labels
    // JOIN kube_pod_labels with container metrics
    // container_cpu_usage_seconds_total comes from cAdvisor (embedded in Kubelet).
    // It knows about low-level details like pod, namespace, and image, but it is unaware of your high-level Kubernetes labels

    let cpu_query = r#"
        sum(
            rate(
                container_cpu_usage_seconds_total{container!="",container!="POD",namespace=~"user-.*"}[5m]
            )
            * on(pod, namespace) group_left(label_poddle_io_deployment_id, label_poddle_io_project_id)
            kube_pod_labels{label_poddle_io_managed_by="poddle"}
        ) by (pod, namespace, label_poddle_io_deployment_id, label_poddle_io_project_id)
    "#;

    let memory_query = r#"
        sum(
            container_memory_working_set_bytes{container!="",container!="POD",namespace=~"user-.*"}
            * on(pod, namespace) group_left(label_poddle_io_deployment_id, label_poddle_io_project_id)
            kube_pod_labels{label_poddle_io_managed_by="poddle"}
        ) by (pod, namespace, label_poddle_io_deployment_id, label_poddle_io_project_id)
    "#;

    // Execute queries
    let (cpu_result, memory_result) = tokio::try_join!(
        client.query(cpu_query).get(),
        client.query(memory_query).get()
    )
    .map_err(|e| {
        error!(error = %e, "Prometheus query failed");
        AppError::InternalServerError(format!("Prometheus query failed: {}", e))
    })?;

    let query_duration = start.elapsed();
    info!(
        duration_ms = query_duration.as_millis(),
        "Prometheus query completed"
    );

    if query_duration > Duration::from_secs(5) {
        tracing::warn!("Prometheus query is taking dangerously long!");
    }

    // info!("cpu_result: {:?}", cpu_result);
    // info!("memory_result: {:?}", memory_result);

    // Aggregate Data
    // Structure: ProjectID -> DeploymentID -> Values
    let mut project_map: HashMap<String, HashMap<String, MetricSnapshot>> = HashMap::new();
    let now = Utc::now().timestamp();

    // Helper closure to process vector results
    let mut process_vector = |data: &Data, is_cpu: bool| {
        if let Data::Vector(vec) = data {
            for instant_vector in vec {
                let metric = instant_vector.metric();

                // Safely extract labels
                if let (Some(project_id), Some(deployment_id)) = (
                    metric.get("label_poddle_io_project_id"),
                    metric.get("label_poddle_io_deployment_id"),
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
    let pipeline_start = std::time::Instant::now();

    let mut pipe = redis::pipe();
    let metric_snapshots_to_keep = cfg.metric_snapshots_to_keep;
    let mut total_deployments = 0;

    for (project_id, deployment_map) in project_map {
        let mut deployment_metrics: Vec<DeploymentMetricUpdate> = Vec::new();

        for (deployment_id, aggregated_value) in deployment_map {
            total_deployments += 1;

            // Add to Project Payload
            deployment_metrics.push(DeploymentMetricUpdate {
                deployment_id: deployment_id.clone(),
                cpu: aggregated_value.cpu,
                memory: aggregated_value.memory,
            });

            // Ensure key exists
            let key = CacheKeys::deployment_metrics(&deployment_id);
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
            let ttl = cfg.scrape_interval_seconds * cfg.metric_snapshots_to_keep as u64;
            pipe.expire(&key, ttl.try_into().unwrap()).ignore();
        }

        // Publish Project Batch Message
        if !deployment_metrics.is_empty() {
            let channel = ChannelNames::project_metrics(&project_id);
            let message = ComputeEvent::MetricsUpdate {
                deployments: deployment_metrics,
            };

            if let Ok(payload) = serde_json::to_string(&message) {
                pipe.publish(channel, payload).ignore();
            }
        }
    }

    // Execute Pipeline
    if total_deployments > 0 {
        let _: () = pipe
            .query_async(&mut redis.connection)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Redis pipeline failed: {}", e)))?;

        info!(
            deployments_updated = total_deployments,
            duration_ms = pipeline_start.elapsed().as_millis(),
            "✅ Redis pipeline executed"
        );
    }

    Ok(())
}
