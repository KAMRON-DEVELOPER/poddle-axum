#[tokio::main]
async fn main() {
    let redis = RedisClient::new().await;
    let k8s = kube::Client::try_default().await?;
    let prometheus = PrometheusClient::new("http://prometheus:9090");

    // Spawn 3 independent tasks
    tokio::try_join!(
        event_watcher(k8s.clone(), redis.clone()),
        periodic_syncer(k8s.clone(), redis.clone()),
        metrics_scraper(prometheus, redis.clone())
    )?;
}

// TASK 1: Real-time event watching
async fn event_watcher(k8s: Client, redis: RedisClient) {
    let deploys = Api::<Deployment>::all(k8s.clone());
    let pods = Api::<Pod>::all(k8s.clone());

    let deploy_stream = watcher(deploys, Config::default()).applied_objects();
    let pod_stream = watcher(pods, Config::default()).applied_objects();

    // Merge streams and handle events
    tokio::select! {
        _ = handle_deployment_events(deploy_stream, redis.clone()) => {},
        _ = handle_pod_events(pod_stream, redis.clone()) => {},
    }
}

// TASK 2: Periodic full reconciliation
async fn periodic_syncer(k8s: Client, redis: RedisClient) {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 min

    loop {
        interval.tick().await;

        info!("Running full reconciliation...");

        // List ALL deployments across all user namespaces
        let deploys = Api::<Deployment>::all(k8s.clone())
            .list(&ListParams::default().labels("app.poddle.io/managed=true"))
            .await?;

        for deploy in deploys {
            let key = format!("deploy:{}:{}", deploy.namespace()?, deploy.name_any());
            let current_state = serde_json::to_string(&deploy)?;

            // Compare with cache
            let cached: Option<String> = redis.get(&key).await?;

            if cached.as_ref() != Some(&current_state) {
                warn!("Drift detected for {}, reconciling", key);
                redis.set(&key, &current_state).await?;

                // Optional: Update Postgres too
                db.upsert_deployment_status(&deploy).await?;
            }
        }

        info!("Reconciliation complete");
    }
}

// TASK 3: Prometheus metrics scraping
async fn metrics_scraper(prom: PrometheusClient, redis: RedisClient) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        interval.tick().await;

        // Scrape CPU usage per deployment
        let cpu_query = r#"
            sum by (namespace, deployment) (
                rate(container_cpu_usage_seconds_total{container!=""}[1m])
            )
        "#;

        let results = prom.query(cpu_query).await?;

        for metric in results {
            let key = format!("metrics:{}:{}:cpu", metric.namespace, metric.deployment);
            redis.set_ex(&key, metric.value, 60).await?; // Expire in 60s
        }
    }
}
