use chrono::Utc;
use compute_core::{
    cache_keys::CacheKeys,
    channel_names::ChannelNames,
    configs::PrometheusConfig,
    event::ComputeEvent,
    schemas::{DeploymentMetricUpdate, MetricSnapshot, PodHistory, PodMetricUpdate, PodPhase},
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
        cfg.scrape_interval, cfg.snapshots_to_keep, cfg.rate
    );

    let mut interval = tokio::time::interval(Duration::from_secs(cfg.scrape_interval as u64));

    loop {
        interval.tick().await;

        if let Err(e) = scrape(&cfg, &client, redis.clone()).await {
            error!("❌ Failed to scrape metrics: {}", e);
        }
    }
}

#[derive(Default, Debug)]
struct DeploymentBuffer {
    snapshot: MetricSnapshot,
    pod_map: HashMap<String, PodBuffer>,
}

#[derive(Default, Debug)]
struct PodBuffer {
    uid: String,
    phase: PodPhase,
    restart_count: i32,
    snapshot: MetricSnapshot,
}

/// labels are stored in `kube-state-metrics` and it exports a specific metric called `kube_pod_labels`
/// JOIN `kube_pod_labels` with container metrics
/// `container_cpu_usage_seconds_total` comes from `cAdvisor` (embedded in `Kubelet`).
/// It knows about low-level details like `pod`, `namespace`, and `image`, but it is unaware of your high-level Kubernetes `labels`
#[tracing::instrument("scrape", skip_all, fields(scrape_id  = tracing::field::Empty), err)]
async fn scrape(cfg: &PrometheusConfig, client: &Client, mut redis: Redis) -> Result<(), AppError> {
    let scrape_id = Uuid::new_v4();
    tracing::Span::current().record("scrape_id", &scrape_id.to_string());

    // kube_pod_info
    // kube_pod_labels
    // kube_pod_status_phase

    // For every pod, kube-state-metrics emits one time series per phase:
    //  kube_pod_status_phase{pod="p1", phase="Pending"}   0
    //  kube_pod_status_phase{pod="p1", phase="Running"}   1
    //  kube_pod_status_phase{pod="p1", phase="Succeeded"} 0
    //  kube_pod_status_phase{pod="p1", phase="Failed"}    0
    //  kube_pod_status_phase{pod="p1", phase="Unknown"}   0
    // Exactly one of them has value 1.
    // All others are 0.

    let cpu_query = format!(
        r#"
        (
            sum(
                rate(
                    container_cpu_usage_seconds_total{{
                        container!="",
                        container!="POD",
                        namespace=~"user-.*"
                    }}[{}]
                )
            ) by (pod, namespace)
        )
        * on(pod, namespace) group_left(uid) kube_pod_info
        * on(pod, namespace) group_left(phase) (kube_pod_status_phase == 1)
        * on(pod, namespace) group_left(label_poddle_io_project_id, label_poddle_io_deployment_id) kube_pod_labels{{label_poddle_io_managed_by="poddle"}}
        "#,
        cfg.rate
    );

    let memory_query = r#"
        sum(
            container_memory_working_set_bytes{
                container!="",
                container!="POD",
                namespace=~"user-.*"
            }
        ) by (pod, namespace)
        * on(pod, namespace) group_left(label_poddle_io_project_id, label_poddle_io_deployment_id) kube_pod_labels{label_poddle_io_managed_by="poddle"}
    "#;

    let restarts_query = r#"
        sum(
            kube_pod_container_status_restarts_total
        ) by (pod, namespace)
        * on(pod, namespace) group_left(label_poddle_io_project_id, label_poddle_io_deployment_id) kube_pod_labels{label_poddle_io_managed_by="poddle"}
    "#;

    // Execute queries
    let start = std::time::Instant::now();

    let (cpu_res, mem_res, restart_res) = tokio::try_join!(
        client.query(cpu_query).get(),
        client.query(memory_query).get(),
        client.query(restarts_query).get()
    )
    .map_err(|e| {
        error!(error = %e, "❌ Prometheus query failed");
        AppError::InternalServerError(format!("Prometheus query failed: {}", e))
    })?;

    println!("cpu_res: {:#?}\n", cpu_res);
    println!("mem_res: {:#?}\n", mem_res);
    println!("restart_res: {:#?}\n", restart_res);

    info!(
        elapsed = start.elapsed().as_millis(),
        "🏁 Prometheus query completed"
    );

    /*
    {
        ProjectID: {
            DeploymentID: DeploymentBuffer {
                snapshot: MetricSnapshot,
                pod_map: {
                    name: PodBuffer {
                        uid: ...,
                        phase: ...,
                        restart_count: ...,
                        snapshot: MetricSnapshot
                    }
                }
            }
        }
    }
    */
    let mut project_map: HashMap<String, HashMap<String, DeploymentBuffer>> = HashMap::new();

    // Process CPU (This now includes UID and Phase labels)
    if let Data::Vector(vecs) = cpu_res.data() {
        for vec in vecs {
            let labels = vec.metric();
            if let (Some(name), Some(uid), Some(pid), Some(did)) = (
                labels.get("pod"),
                labels.get("uid"),
                labels.get("label_poddle_io_project_id"),
                labels.get("label_poddle_io_deployment_id"),
            ) {
                // Phase may not be available
                let phase = labels.get("phase").map(|s| s.as_str()).unwrap_or("Unknown");

                let val = vec.sample().value();
                let ts = vec.sample().timestamp();

                let deployment_entry = project_map
                    .entry(pid.clone())
                    .or_default()
                    .entry(did.clone())
                    .or_default();
                let pod_entry = deployment_entry.pod_map.entry(name.clone()).or_default();

                pod_entry.uid = uid.to_string();
                pod_entry.phase = phase.into();
                pod_entry.snapshot.ts = ts as i64;
                pod_entry.snapshot.cpu += val * 1000.0;

                deployment_entry.snapshot.ts = ts as i64;
                deployment_entry.snapshot.cpu += val * 1000.0;
            }
        }
    }

    // Process Memory
    if let Data::Vector(vecs) = mem_res.data() {
        for vec in vecs {
            let labels = vec.metric();
            if let (Some(name), Some(pid), Some(did)) = (
                labels.get("pod"),
                labels.get("label_poddle_io_project_id"),
                labels.get("label_poddle_io_deployment_id"),
            ) {
                let val = vec.sample().value();
                // Ensure entry exists (it should from CPU, but safe to default)
                let deployment_entry = project_map
                    .entry(pid.clone())
                    .or_default()
                    .entry(did.clone())
                    .or_default();
                let pod_entry = deployment_entry.pod_map.entry(name.clone()).or_default();

                let mem_mb = val / 1024.0 / 1024.0;
                pod_entry.snapshot.memory += mem_mb;
                deployment_entry.snapshot.memory += mem_mb;
            }
        }
    }

    // Process Restarts
    if let Data::Vector(vecs) = restart_res.data() {
        for vec in vecs {
            let labels = vec.metric();
            if let (Some(name), Some(pid), Some(did)) = (
                labels.get("pod"),
                labels.get("label_poddle_io_project_id"),
                labels.get("label_poddle_io_deployment_id"),
            ) {
                let val = vec.sample().value();
                let deployment_entry = project_map
                    .entry(pid.clone())
                    .or_default()
                    .entry(did.clone())
                    .or_default();
                let pod_entry = deployment_entry.pod_map.entry(name.clone()).or_default();

                pod_entry.restart_count = val as i32;
            }
        }
    }

    println!("project_map: {:#?}", project_map);

    let mut projects_count = 0;
    let mut deployments_count = 0;
    let mut pods_count = 0;

    let mut p = redis::pipe();

    for (id, deployment_map) in project_map {
        projects_count += 1;
        let mut deployment_messages = Vec::new();

        // We send deployment messages after deployment_map loop
        for (id, DeploymentBuffer { snapshot, pod_map }) in deployment_map {
            deployments_count += 1;
            let mut pod_messages = Vec::new();

            // Deployment Metrics Key
            let key = CacheKeys::deployment_metrics(&id);
            let ttl = cfg.scrape_interval * cfg.snapshots_to_keep;

            // Append Snapshot
            p.lpush(&key, &snapshot).ignore();
            p.ltrim(&key, -cfg.snapshots_to_keep as isize, -1).ignore();
            p.expire(&key, ttl).ignore();

            // We send pod messages after pod_map loop
            for (
                name,
                PodBuffer {
                    uid,
                    phase,
                    restart_count,
                    snapshot,
                },
            ) in pod_map
            {
                pods_count += 1;
                let index_key = CacheKeys::deployment_pods(&id);
                let meta_key = CacheKeys::deployment_pod_meta(&id, &uid);
                let metrics_key = CacheKeys::deployment_pod_metrics(&id, &uid);

                // Index
                let score = Utc::now().timestamp();
                p.zadd(&index_key, &uid, score).ignore();
                p.expire(&index_key, ttl).ignore();

                // Metadata
                let meta = PodHistory {
                    uid,
                    name,
                    phase,
                    restart_count,
                };
                p.cmd("HSET").arg(&meta_key).arg(&meta).ignore();
                p.expire(&meta_key, ttl).ignore();
                // p.hset_multiple(
                //     &meta_key,
                //     &[
                //         ("uid", &uid),
                //         ("name", &name),
                //         ("phase", &phase),
                //         ("restarts", &restarts.to_string()),
                //     ],
                // )
                // .ignore();

                // Append snapshots
                p.lpush(&metrics_key, &snapshot).ignore();
                p.ltrim(&metrics_key, -cfg.snapshots_to_keep as isize, -1)
                    .ignore();
                p.expire(&metrics_key, ttl).ignore();

                pod_messages.push(PodMetricUpdate { meta, snapshot });
            }

            // Publish pod metrics update message to deployment page
            if !pod_messages.is_empty() {
                let channel = ChannelNames::deployment_metrics(&id);
                let message = ComputeEvent::PodMetricsUpdate {
                    updates: pod_messages,
                };
                if let Ok(message) = serde_json::to_string(&message) {
                    p.publish(channel, message).ignore();
                }
            }

            // We can use id cleanly after all referances
            deployment_messages.push(DeploymentMetricUpdate { id, snapshot });
        }

        // Publish deployment metrics update message to project page
        if !deployment_messages.is_empty() {
            let channel = ChannelNames::deployments_metrics(&id);
            let message = ComputeEvent::DeploymentMetricsUpdate {
                updates: deployment_messages,
            };
            if let Ok(message) = serde_json::to_string(&message) {
                p.publish(channel, message).ignore();
            }
        }
    }

    // Execute Pipeline
    if deployments_count > 0 {
        let start = std::time::Instant::now();
        // We use `turbofish` syntax instead `let _: ()`
        p.query_async::<()>(&mut redis.con).await.map_err(|e| {
            AppError::InternalServerError(format!("❌ Redis pipeline failed: {}", e))
        })?;

        info!(
            projects_count = projects_count,
            deployments_count = deployments_count,
            pods_count = pods_count,
            elapsed = start.elapsed().as_millis(),
            "✅ Deployments scraped"
        );
    } else {
        info!("⏸️ No deployment to scrape");
    }

    Ok(())
}
