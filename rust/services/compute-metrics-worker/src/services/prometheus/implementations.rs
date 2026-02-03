use compute_core::{
    cache_keys::CacheKeys,
    channel_names::ChannelNames,
    configs::PrometheusConfig,
    event::ComputeEvent,
    schemas::{
        DeploymentMetricUpdate, MetricHistory, MetricSnapshot, PodMetricHistory, PodMetricUpdate,
        PodPhase,
    },
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
        "⚙️  Scrape interval: {}s, Snapshot to keep: {}, rate: {}",
        cfg.scrape_interval_seconds, cfg.metric_snapshots_to_keep, cfg.rate
    );

    let mut interval = tokio::time::interval(Duration::from_secs(cfg.scrape_interval_seconds));

    loop {
        interval.tick().await;

        if let Err(e) = scrape(&cfg, &client, redis.clone()).await {
            error!("❌ Failed to scrape metrics: {}", e);
        }
    }
}

#[derive(Default)]
struct ScrapedDeployment {
    snapshot: MetricSnapshot,
    pod_map: HashMap<String, MetricSnapshot>,
}

/// labels are stored in `kube-state-metrics` and it exports a specific metric called `kube_pod_labels`
/// JOIN `kube_pod_labels` with container metrics
/// `container_cpu_usage_seconds_total` comes from `cAdvisor` (embedded in `Kubelet`).
/// It knows about low-level details like `pod`, `namespace`, and `image`, but it is unaware of your high-level Kubernetes `labels`
#[tracing::instrument("scrape", skip_all, fields(scrape_id  = tracing::field::Empty), err)]
async fn scrape(cfg: &PrometheusConfig, client: &Client, mut redis: Redis) -> Result<(), AppError> {
    let scrape_id = Uuid::new_v4();
    tracing::Span::current().record("scrape_id", &scrape_id.to_string());

    let cpu_query = format!(
        r#"
        sum(
            rate(
                container_cpu_usage_seconds_total{{container!="",container!="POD",namespace=~"user-.*"}}[{}]
            )
            * on(pod, namespace) group_left(label_poddle_io_deployment_id, label_poddle_io_project_id)
            kube_pod_labels{{label_poddle_io_managed_by="poddle"}}
        ) by (pod, namespace, label_poddle_io_deployment_id, label_poddle_io_project_id)
        "#,
        cfg.rate
    );
    let memory_query = r#"
        sum(
            container_memory_working_set_bytes{container!="",container!="POD",namespace=~"user-.*"}
            * on(pod, namespace) group_left(label_poddle_io_deployment_id, label_poddle_io_project_id)
            kube_pod_labels{label_poddle_io_managed_by="poddle"}
        ) by (pod, namespace, label_poddle_io_deployment_id, label_poddle_io_project_id)
    "#;

    // Execute queries
    let query_start = std::time::Instant::now();
    let (cpu_result, memory_result) = tokio::try_join!(
        client.query(cpu_query).get(),
        client.query(memory_query).get()
    )
    .map_err(|e| {
        error!(error = %e, "❌ Prometheus query failed");
        AppError::InternalServerError(format!("Prometheus query failed: {}", e))
    })?;

    let query_elapsed = query_start.elapsed();
    info!(
        query_elapsed = query_elapsed.as_millis(),
        "🏁 Prometheus query completed"
    );
    if query_elapsed > Duration::from_secs(5) {
        tracing::warn!("⚠️ Prometheus query is taking dangerously long!");
    }

    // Structure: { ProjectID: { DeploymentID: { aggregated: MetricSnapshot, pods: { name: MetricSnapshot } } } }
    let mut project_map: HashMap<String, HashMap<String, ScrapedDeployment>> = HashMap::new();
    // Structure: { DeploymentID: { aggregated: MetricSnapshot, pods: { name: MetricSnapshot } } }
    // let mut deployment_map: HashMap<String, ScrapedDeployment> = HashMap::new();

    // Helper closure to process vector results
    let mut process_data = |data: &Data, is_cpu: bool| {
        if let Data::Vector(vecs) = data {
            for vec in vecs {
                let labels = vec.metric();

                // Safely extract labels
                if let (Some(project_id), Some(deployment_id), Some(pod_name)) = (
                    labels.get("label_poddle_io_project_id"),
                    labels.get("label_poddle_io_deployment_id"),
                    labels.get("pod"),
                ) {
                    let value = vec.sample().value();
                    let ts = vec.sample().timestamp();

                    let deployment_entry = project_map
                        .entry(project_id.clone())
                        .or_default()
                        .entry(deployment_id.clone())
                        .or_default();

                    let pod_entry = deployment_entry
                        .pod_map
                        .entry(pod_name.clone())
                        .or_default();

                    deployment_entry.snapshot.ts = ts as i64;
                    pod_entry.ts = ts as i64;
                    if is_cpu {
                        // CPU is usually in cores, multiply by 1000 for millicores
                        deployment_entry.snapshot.cpu += value * 1000.0;
                        pod_entry.cpu += value * 1000.0;
                    } else {
                        // Memory is in bytes, convert to MB if needed, or keep bytes.
                        deployment_entry.snapshot.memory += value / 1024.0 / 1024.0;
                        pod_entry.memory += value / 1024.0 / 1024.0;
                    }
                }
            }
        }
    };

    process_data(cpu_result.data(), true);
    process_data(memory_result.data(), false);

    // Pipeline to Redis
    let pipeline_start = std::time::Instant::now();
    let mut p = redis::pipe();

    let mut projects_count = 0;
    let mut deployments_count = 0;
    let mut pods_count = 0;

    for (id, deployment_map) in project_map {
        projects_count += 1;
        let mut deployment_updates = Vec::new();

        for (id, ScrapedDeployment { snapshot, pod_map }) in deployment_map {
            deployments_count += 1;
            let mut pod_updates = Vec::new();
            let key = CacheKeys::deployment_metrics(&id);
            let ttl = cfg.scrape_interval_seconds as i64 * cfg.metric_snapshots_to_keep;

            // Ensures $.history and $.pods exist
            let initial = MetricHistory {
                history: vec![],
                pods: HashMap::new(),
            };
            p.cmd("JSON.SET")
                .arg(&key)
                .arg("$")
                .arg(&initial)
                .arg("NX")
                .ignore();

            // Update Aggregated History
            p.json_arr_append(&key, "$.history", &snapshot)?;
            p.json_arr_trim(&key, "$.history", -cfg.metric_snapshots_to_keep, -1)?;
            p.expire(&key, ttl).ignore();

            deployment_updates.push(DeploymentMetricUpdate {
                id: id.clone(),
                snapshot,
            });

            for (name, snapshot) in pod_map {
                pods_count += 1;
                // Safe JSON Path for map keys: $.pods["pod-name"]
                let pod_path = format!("$.pods[\"{}\"]", name);
                let history_path = format!("{}.history", pod_path);

                // TODO: Get real phase from K8s/kube-state-metrics if available in query
                let phase = PodPhase::Running;
                let initial = PodMetricHistory {
                    name: name.clone(),
                    phase: phase.clone(),
                    history: vec![],
                };

                p.cmd("JSON.SET")
                    .arg(&key)
                    .arg(&pod_path)
                    .arg("$")
                    .arg(&initial)
                    .arg("NX")
                    .ignore();

                // Append History & Trim
                p.json_arr_append(&key, &history_path, &snapshot)?;
                p.json_arr_trim(&key, &history_path, -cfg.metric_snapshots_to_keep, -1)?;

                pod_updates.push(PodMetricUpdate {
                    name,
                    phase,
                    snapshot,
                });
            }

            // Publish pod metric update message to deployment page
            if !pod_updates.is_empty() {
                let channel = ChannelNames::deployment_metrics(&id);
                let message = ComputeEvent::PodMetricsUpdate {
                    updates: pod_updates,
                };
                if let Ok(message) = serde_json::to_string(&message) {
                    p.publish(channel, message).ignore();
                }
            }
        }

        // Publish deployment metric update message to project page
        if !deployment_updates.is_empty() {
            let channel = ChannelNames::project_metrics(&id);
            let message = ComputeEvent::DeploymentMetricsUpdate {
                updates: deployment_updates,
            };
            if let Ok(message) = serde_json::to_string(&message) {
                p.publish(channel, message).ignore();
            }
        }
    }

    // Execute Pipeline
    if deployments_count > 0 {
        // We use `turbofish` syntax instead `let _: ()`
        p.query_async::<()>(&mut redis.connection)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("❌ Redis pipeline failed: {}", e))
            })?;

        info!(
            projects_count = projects_count,
            deployments_count = deployments_count,
            pods_count = pods_count,
            pipeline_elapsed = pipeline_start.elapsed().as_millis(),
            "✅ Deployments scraped"
        );
    } else {
        info!(
            pipeline_elapsed = pipeline_start.elapsed().as_millis(),
            "⏸️ No deployment to scrape"
        );
    }

    Ok(())
}
