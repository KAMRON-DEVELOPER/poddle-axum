use std::time::Duration;

use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use kube::runtime::{WatchStreamExt, reflector, watcher};
use kube::{Api, Client, ResourceExt};
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;
use shared::models::DeploymentStatus;
use shared::utilities::config::Config;
use shared::utilities::errors::AppError;
use sqlx::PgPool;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Watches ALL K8s deployments and syncs status to DB
pub async fn deployment_status_syncer(
    pool: PgPool,
    client: Client,
    mut connection: MultiplexedConnection,
) -> Result<(), AppError> {
    info!("Starting deployment status syncer");

    // Watch deployments in all namespaces
    // let api: Api<Deployment> = Api::all(client);
    let api = Api::<Deployment>::all(client);

    // Create a reflector store (efficient caching)
    let (reader, writer) = reflector::store::<Deployment>();

    // Watch for changes
    let watch_config = watcher::Config::default().labels("managed-by=poddle"); // Only watch our deployments

    // stream: Pin<Box<dyn Stream<Item = Result<Deployment, Error>> + Send>>
    // let Some(event: Deployment)
    let mut stream = reflector(writer, watcher(api, watch_config))
        .default_backoff()
        // .reflect(writer)
        .applied_objects()
        .boxed();

    // stream: Pin<Box<dyn Stream<Item = Result<Event<Deployment>, Error>> + Send>>
    // let Some(event: Event<Deployment>)
    // let mut stream = reflector(writer, watcher(api, Default::default())).boxed();

    tokio::pin!(stream);

    while let Some(event) = stream.next().await {
        match event {
            Ok(deployment) => {
                if let Err(e) = process_deployment_status(&pool, &deployment).await {
                    error!("Failed to process deployment status: {}", e);
                }
            }
            Err(e) => {
                error!("Watcher error: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    // while let Some(event) = stream.try_next().await? {
    //     use kube::runtime::watcher::Event;

    //     match event {
    //         Event::Apply(d) | Event::InitApply(d) => {
    //             info!(
    //                 "Syncing deployment: {}/{}",
    //                 d.namespace().unwrap_or("default".to_string()),
    //                 d.name_any()
    //             );
    //             update_redis(&mut connection, &d).await?;
    //         }
    //         Event::Delete(d) => {
    //             info!(
    //                 "Deleting deployment from cache: {}/{}",
    //                 d.namespace().unwrap_or("default".to_string()),
    //                 d.name_any()
    //             );
    //             let key = format!(
    //                 "deploy:{}:{}",
    //                 d.namespace().unwrap_or("default".to_string()),
    //                 d.name_any()
    //             );
    //             let _: u64 = connection.del(&key).await?;
    //         }
    //         Event::Init => info!("Starting full sync..."),
    //         Event::InitDone => info!("Full sync complete"),
    //     }
    // }

    Ok(())
}

async fn update_redis(
    connection: &mut MultiplexedConnection,
    deployment: &Deployment,
) -> Result<(), AppError> {
    let namespace = deployment.namespace().unwrap_or("default".to_owned());
    let name = deployment.name_any();
    let key = format!("deploy:{}:{}", namespace, name);

    let state = serde_json::json!({
        "name": name,
        "namespace": namespace,
        "replicas": deployment.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1),
        "ready_replicas": deployment.status.as_ref().and_then(|s| s.ready_replicas).unwrap_or(0),
    });

    let _: u64 = connection.set_ex(&key, state.to_string(), 600).await?; // 10 min TTL

    Ok(())
}

/// Process a single K8s deployment and update DB status
async fn process_deployment_status(pool: &PgPool, deployment: &Deployment) -> Result<(), AppError> {
    let name = deployment.name_any();
    let namespace = deployment
        .metadata
        .namespace
        .as_ref()
        .ok_or_else(|| AppError::NamaSpaceError("Namespace not found".to_string()))?;

    // Extract deployment-id from labels
    let deployment_id = deployment
        .metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get("deployment-id"))
        .and_then(|id| Uuid::parse_str(id).ok());

    let Some(deployment_id) = deployment_id else {
        warn!(
            "Deployment {}/{} has no deployment-id label",
            namespace, name
        );
        return Ok(());
    };

    // Get K8s deployment status
    let status = deployment.status.as_ref();

    let desired_replicas = deployment
        .spec
        .as_ref()
        .and_then(|s| s.replicas)
        .unwrap_or(1);

    let ready_replicas = status.and_then(|s| s.ready_replicas).unwrap_or(0);

    let available_replicas = status.and_then(|s| s.available_replicas).unwrap_or(0);

    let unavailable_replicas = status.and_then(|s| s.unavailable_replicas).unwrap_or(0);

    // Determine deployment status based on K8s state
    let new_status = determine_status(
        desired_replicas,
        ready_replicas,
        available_replicas,
        unavailable_replicas,
        status,
    );

    // Check if status changed in DB
    let current_db_status: Option<String> =
        sqlx::query_scalar("SELECT status::TEXT FROM deployments WHERE id = $1")
            .bind(deployment_id)
            .fetch_optional(pool)
            .await?;

    let status_changed = current_db_status
        .as_ref()
        .map(|s| s != &new_status.to_string())
        .unwrap_or(true);

    if status_changed {
        info!(
            "Deployment {} status: {} -> {}",
            deployment_id,
            current_db_status.as_deref().unwrap_or("unknown"),
            new_status
        );

        // Update status in DB
        sqlx::query(
            r#"
            UPDATE deployments 
            SET status = $1::deployment_status,
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(new_status.to_string())
        .bind(deployment_id)
        .execute(pool)
        .await?;

        // Log event
        sqlx::query(
            r#"
            INSERT INTO deployment_events (deployment_id, event_type, message)
            VALUES ($1, 'status_changed', $2)
            "#,
        )
        .bind(deployment_id)
        .bind(format!(
            "Status changed to: {} (Ready: {}/{})",
            new_status, ready_replicas, desired_replicas
        ))
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Determine deployment status based on K8s state
fn determine_status(
    desired: i32,
    ready: i32,
    available: i32,
    unavailable: i32,
    status: Option<&k8s_openapi::api::apps::v1::DeploymentStatus>,
) -> DeploymentStatus {
    // Check conditions
    let conditions = status.and_then(|s| s.conditions.as_ref());

    let is_progressing = conditions
        .and_then(|conds| conds.iter().find(|c| c.type_ == "Progressing"))
        .map(|c| c.status == "True")
        .unwrap_or(false);

    let has_available = conditions
        .and_then(|conds| conds.iter().find(|c| c.type_ == "Available"))
        .map(|c| c.status == "True")
        .unwrap_or(false);

    // Status determination logic
    match (ready, desired, unavailable, is_progressing, has_available) {
        // All replicas ready and available
        (r, d, 0, _, true) if r == d => DeploymentStatus::Healthy,

        // Some replicas ready, but not all (partial availability)
        (r, d, _, _, _) if r > 0 && r < d => DeploymentStatus::Degraded,

        // No replicas ready, but progressing (starting up)
        (0, _, _, true, false) => DeploymentStatus::Starting,

        // Replicas exist but unhealthy
        (0, _, u, false, false) if u > 0 => DeploymentStatus::Unhealthy,

        // Currently scaling (transitioning between replica counts)
        (r, d, _, true, _) if r != d => DeploymentStatus::Scaling,

        // Complete failure (no progress, no availability)
        (0, _, _, false, false) => DeploymentStatus::Failed,

        // Default to provisioning if unclear
        _ => DeploymentStatus::Provisioning,
    }
}

/// Periodically scrape Prometheus metrics for billing
async fn scrape_metrics_periodically(pool: PgPool, config: Config) {
    let mut ticker = interval(Duration::from_secs(3600)); // Every hour

    loop {
        ticker.tick().await;

        info!("Starting metrics scrape for billing");

        if let Err(e) = scrape_and_bill(&pool, &config).await {
            error!("Failed to scrape metrics: {}", e);
        }
    }
}

/// Scrape Prometheus and create billing records
async fn scrape_and_bill(pool: &PgPool, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    // Query Prometheus for CPU/Memory usage per deployment
    let query = r#"
        sum by (deployment_id) (
            rate(container_cpu_usage_seconds_total{
                pod=~".*-.*",
                namespace=~"user-.*"
            }[1h])
        ) * 1000
    "#;

    let url = format!(
        "{}/api/v1/query?query={}",
        config.prometheus_url,
        urlencoding::encode(query)
    );

    let response: serde_json::Value = client.get(&url).send().await?.json().await?;

    let results = response["data"]["result"]
        .as_array()
        .ok_or("Invalid Prometheus response")?;

    for result in results {
        let deployment_id = result["metric"]["deployment_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok());

        let cpu_millicores = result["value"][1]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| v as i32);

        if let (Some(deployment_id), Some(cpu_millicores)) = (deployment_id, cpu_millicores) {
            // Get deployment resources from DB
            let resources: serde_json::Value =
                sqlx::query_scalar("SELECT resources FROM deployments WHERE id = $1")
                    .bind(deployment_id)
                    .fetch_one(pool)
                    .await?;

            let memory_mb = resources["memory_limit_mb"].as_i64().unwrap_or(512) as i32;

            // Calculate cost (example: $0.01 per CPU-hour, $0.005 per GB-hour)
            let cpu_cost = (cpu_millicores as f64 / 1000.0) * 0.01;
            let mem_cost = (memory_mb as f64 / 1024.0) * 0.005;
            let cost_per_hour = cpu_cost + mem_cost;

            // Get user_id
            let user_id: Uuid = sqlx::query_scalar("SELECT user_id FROM deployments WHERE id = $1")
                .bind(deployment_id)
                .fetch_one(pool)
                .await?;

            // Create billing record
            let billing_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO billings (
                    user_id, deployment_id, resources_snapshot,
                    cpu_millicores, memory_mb, cost_per_hour, hours_used
                )
                VALUES ($1, $2, $3, $4, $5, $6, 1.0)
                RETURNING id
                "#,
            )
            .bind(user_id)
            .bind(deployment_id)
            .bind(&resources)
            .bind(cpu_millicores)
            .bind(memory_mb)
            .bind(cost_per_hour)
            .fetch_one(pool)
            .await?;

            // Get balance_id and create transaction
            let balance_id: Uuid = sqlx::query_scalar("SELECT id FROM balances WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(pool)
                .await?;

            // Charge user (negative amount)
            sqlx::query(
                r#"
                INSERT INTO transactions (balance_id, amount, type, detail, billing_id)
                VALUES ($1, $2, 'usage_charge', $3, $4)
                "#,
            )
            .bind(balance_id)
            .bind(-cost_per_hour) // Negative = charge
            .bind(format!("Hourly charge for deployment {}", deployment_id))
            .bind(billing_id)
            .execute(pool)
            .await?;

            info!(
                "Billed user {} for deployment {}: ${:.4}",
                user_id, deployment_id, cost_per_hour
            );
        }
    }

    Ok(())
}
