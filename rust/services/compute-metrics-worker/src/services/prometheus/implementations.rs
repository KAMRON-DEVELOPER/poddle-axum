use crate::{error::AppError, services::prometheus::Prometheus};
use compute_core::{
    cache_keys::CacheKeys,
    channel_names::ChannelNames,
    configs::PrometheusConfig,
    event::ComputeEvent,
    schemas::{DeploymentMetricUpdate, MetricSnapshot, PodMeta, PodMetricUpdate, PodPhase},
};
use factory::factories::redis::Redis;
use prometheus_http_query::{Client, response::Data};
use redis::AsyncTypedCommands;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::{debug, error, info};
use uuid::Uuid;

pub async fn start_metrics_scraper(redis: Redis, prometheus: Prometheus) -> Result<(), AppError> {
    let client = prometheus.client;
    let cfg = prometheus.cfg;

    info!("📈 Starting Prometheus metrics scraper");
    info!(
        "⚙️  Scrape interval: {}s, Snapshot to keep: {}, rate: {}",
        cfg.scrape_interval_secs, cfg.snapshots_to_keep, cfg.rate
    );

    let mut interval = tokio::time::interval(Duration::from_secs(cfg.scrape_interval_secs as u64));

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
    name: String,
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

    // We add `unless on(pod, namespace) kube_pod_deletion_timestamp` to ALL queries.
    // This ensures Prometheus doesn't return data for pods that K8s has marked for death.

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
        unless on(pod, namespace) kube_pod_deletion_timestamp
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
    unless on(pod, namespace) kube_pod_deletion_timestamp
    "#;

    let restarts_query = r#"
    sum(
        kube_pod_container_status_restarts_total
    ) by (pod, namespace)
    * on(pod, namespace) group_left(label_poddle_io_project_id, label_poddle_io_deployment_id) kube_pod_labels{label_poddle_io_managed_by="poddle"}
    unless on(pod, namespace) kube_pod_deletion_timestamp
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

    debug!(
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

    // Process CPU
    if let Data::Vector(vecs) = cpu_res.data() {
        for vec in vecs {
            let labels = vec.metric();
            let (Some(uid), Some(name), Some(pid), Some(did)) = (
                labels.get("uid"),
                labels.get("pod"),
                labels.get("label_poddle_io_project_id"),
                labels.get("label_poddle_io_deployment_id"),
            ) else {
                continue;
            };

            if uid.is_empty() {
                continue;
            }

            let phase = labels.get("phase").map(|s| s.as_str()).unwrap_or("Unknown");

            let val = vec.sample().value();
            let ts = vec.sample().timestamp();

            let deployment_entry = project_map
                .entry(pid.clone())
                .or_default()
                .entry(did.clone())
                .or_default();

            let pod = deployment_entry
                .pod_map
                .entry(uid.clone())
                .or_insert_with(|| PodBuffer {
                    name: name.to_string(),
                    phase: phase.into(),
                    restart_count: 0,
                    snapshot: MetricSnapshot::default(),
                });

            pod.snapshot.ts = ts as i64;
            pod.snapshot.cpu += val * 1000.0;

            deployment_entry.snapshot.ts = ts as i64;
            deployment_entry.snapshot.cpu += val * 1000.0;
        }
    }

    // Process Memory
    if let Data::Vector(vecs) = mem_res.data() {
        for vec in vecs {
            let labels = vec.metric();

            let (Some(name), Some(pid), Some(did)) = (
                labels.get("pod"),
                labels.get("label_poddle_io_project_id"),
                labels.get("label_poddle_io_deployment_id"),
            ) else {
                continue;
            };

            let deployment_entry = project_map
                .entry(pid.clone())
                .or_default()
                .entry(did.clone())
                .or_default();

            // Lookup pod by name to find the entry created by CPU query
            let Some(pod) = deployment_entry
                .pod_map
                .values_mut()
                .find(|p| p.name == *name)
            else {
                continue;
            };

            pod.snapshot.memory += vec.sample().value() / 1024.0 / 1024.0;
            deployment_entry.snapshot.memory += pod.snapshot.memory;
        }
    }

    // Process Restarts
    if let Data::Vector(vecs) = restart_res.data() {
        for vec in vecs {
            let labels = vec.metric();
            let (Some(name), Some(pid), Some(did)) = (
                labels.get("pod"),
                labels.get("label_poddle_io_project_id"),
                labels.get("label_poddle_io_deployment_id"),
            ) else {
                continue;
            };

            let deployment_entry = project_map
                .entry(pid.clone())
                .or_default()
                .entry(did.clone())
                .or_default();

            let Some(pod) = deployment_entry
                .pod_map
                .values_mut()
                .find(|p| p.name == *name)
            else {
                continue;
            };

            pod.restart_count = vec.sample().value() as i32;
        }
    }

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

            // Fetch valid UIDs for this deployment from the Redis Index (Managed by Watcher)
            let index_key = CacheKeys::deployment_pods(&id);

            // If this fails, we default to an empty list (skipping all updates is safer than corrupting state)
            let valid_uids: Vec<String> = redis
                .con
                .zrange(&index_key, 0, -1)
                .await
                .unwrap_or_default();

            // Create a HashSet for fast O(1) lookups
            let valid_uid_set: HashSet<String> = valid_uids.into_iter().collect();

            // Deployment Metrics Key
            let key = CacheKeys::deployment_metrics(&id);
            // let ttl = cfg.scrape_interval_secs * cfg.snapshots_to_keep;

            // Append Snapshot
            p.lpush_exists(&key, &snapshot).ignore();
            p.ltrim(&key, -cfg.snapshots_to_keep as isize, -1).ignore();
            // p.expire(&key, ttl).ignore();

            // We send pod messages after pod_map loop
            for (
                uid,
                PodBuffer {
                    name,
                    phase,
                    restart_count,
                    snapshot,
                },
            ) in pod_map
            {
                // Safety Check, event watcher should be already created the pod, we respect that
                if !valid_uid_set.contains(&uid) {
                    // This pod is in Prometheus but NOT in our Watcher index
                    // It is likely a terminating ghost. SKIP IT.
                    continue;
                }

                pods_count += 1;
                let meta_key = CacheKeys::deployment_pod_meta(&id, &uid);
                let metrics_key = CacheKeys::deployment_pod_metrics(&id, &uid);

                // Metadata
                let meta = PodMeta {
                    uid,
                    name,
                    phase,
                    restart_count,
                };
                let items = meta.as_redis_items();
                p.hset_multiple(&meta_key, &items).ignore();
                // p.expire(&meta_key, ttl).ignore();

                // Append snapshots
                p.lpush_exists(&metrics_key, &snapshot).ignore();
                p.ltrim(&metrics_key, -cfg.snapshots_to_keep as isize, -1)
                    .ignore();
                // p.expire(&metrics_key, ttl).ignore();

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

        debug!(
            projects_count = projects_count,
            deployments_count = deployments_count,
            pods_count = pods_count,
            elapsed = start.elapsed().as_millis(),
            "✅ Deployments scraped"
        );
    } else {
        debug!("⏸️ No deployment to scrape");
    }

    Ok(())
}
