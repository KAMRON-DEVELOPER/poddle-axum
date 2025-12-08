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

// Helper struct for aggregation
#[derive(Default)]
struct AggregatedValue {
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
    let cpu_query = r#"
        sum(
            rate(
                container_cpu_usage_seconds_total{
                    container!="",
                    container!="POD",
                    namespace=~"user-.*"
                }[5m]
            )
        ) by (pod, namespace, label_deployment_id)
    "#;
    let memory_query = r#"
        sum(
            container_memory_working_set_bytes{
                container!="",
                container!="POD",
                namespace=~"user-.*"
            }
        ) by (pod, namespace, label_deployment_id)
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

    // Aggregate in Memory
    let mut aggregates: HashMap<String, AggregatedValue> = HashMap::new();
    let now = Utc::now().timestamp();

    // Parse CPU
    if let prometheus_http_query::response::Data::Vector(cpu_vec) = cpu_result.data() {
        for sample in cpu_vec {
            if let Some(dep_id) = sample.metric().get("label_deployment_id") {
                let val = sample.sample().value() * 1000.0;
                aggregates.entry(dep_id.clone()).or_default().cpu += val;
            }
        }
    }

    // Parse Memory
    if let prometheus_http_query::response::Data::Vector(mem_vec) = memory_result.data() {
        for sample in mem_vec {
            if let Some(dep_id) = sample.metric().get("label_deployment_id") {
                let val = sample.sample().value(); // Bytes
                aggregates.entry(dep_id.clone()).or_default().memory += val;
            }
        }
    }

    // Pipeline to Redis
    let mut pipe = redis::pipe();
    let history_limit = config.history_points_to_keep as i64;

    for (deployment_id, values) in &aggregates {
        let key = CacheKeys::deployment_metrics(deployment_id);
        let pubsub_channel = ChannelNames::deployment_metrics(deployment_id);

        let cpu_point = MetricPoint {
            ts: now,
            v: values.cpu,
        };
        let mem_point = MetricPoint {
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
        let _ = pipe.json_arr_append(&key, "$.cpu_history", &cpu_point);
        let _ = pipe.json_arr_append(&key, "$.memory_history", &mem_point);

        let _ = pipe.json_arr_trim(&key, "$.cpu_history", -history_limit, -1);
        let _ = pipe.json_arr_trim(&key, "$.memory_history", -history_limit, -1);

        let message = serde_json::json!({
            "cpu": cpu_point,
            "memory": mem_point
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
