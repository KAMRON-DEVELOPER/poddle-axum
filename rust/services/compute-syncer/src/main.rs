pub mod app;
pub mod config;
pub mod error;
pub mod implementations;
pub mod services;
pub mod utilities;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::result::Result::Ok;

use factory::factories::{
    database::Database, kubernetes::Kubernetes, observability::Observability, redis::Redis,
};

use tokio::task::JoinSet;
use tracing::{error, info};
use utility::shutdown_signal::shutdown_signal;

use crate::{
    config::Config,
    error::AppError,
    services::prometheus::Prometheus,
    utilities::{
        deployment_status_syncer::start_deployment_status_syncer,
        metrics_scraper::start_metrics_scraper, reconcilation_loop::start_reconciliation_loop,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // These are baked at COMPILE time
    let cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // let cargo_crate_name = env!("CARGO_CRATE_NAME");
    let cargo_pkg_name = env!("CARGO_PKG_NAME");
    let cargo_pkg_version = env!("CARGO_PKG_VERSION").into();

    let env_path = cargo_manifest_dir.join(".env");

    // Load workspace root .env as fallback
    dotenvy::dotenv().ok();
    // Load service-specific .env
    dotenvy::from_path(env_path).ok();

    let config = Config::init(cargo_manifest_dir).await?;
    let _guard = Observability::init(
        &config.otel_exporter_otlp_endpoint,
        cargo_pkg_name,
        cargo_pkg_version,
    )
    .await;

    let kubernetes = Kubernetes::new(&config).await?;
    let database = Database::new(&config).await;
    let redis = Redis::new(&config).await;
    // let kafka = Kafka::new(config, "users-service")?;
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let prometheus = Prometheus::new(&config, http_client.clone()).await?;

    let mut set = JoinSet::new();

    // Spawn tasks into the set
    set.spawn(start_deployment_status_syncer(
        database.pool.clone(),
        redis.connection.clone(),
        kubernetes.client.clone(),
    ));
    set.spawn(start_metrics_scraper(
        config.clone(),
        prometheus.client,
        redis,
    ));
    set.spawn(start_reconciliation_loop(
        database.pool.clone(),
        kubernetes.client.clone(),
    ));
    set.spawn(start_health_server(cargo_pkg_name, config.server_address));

    info!("✅ All background tasks started");

    // Unified shutdown logic
    tokio::select! {
        _ = shutdown_signal() => {
            info!("🛑 Shutdown signal received");
            set.shutdown().await;
        }
        Some(result) = set.join_next() => {
            match result {
                Ok(Ok(())) => error!("A background task exited unexpectedly!"),
                Ok(Err(e)) => error!("Task failed: {}", e),
                Err(e) => error!("Task panic: {}", e),
            }
            // Optional: trigger shutdown if a critical task dies
            set.shutdown().await;
        }
    }

    info!("👋 {} shutting down", cargo_pkg_name);

    Ok(())
}

// Start a simple HTTP server for health checks and metrics
async fn start_health_server(cargo_pkg_name: &str, addr: SocketAddr) -> Result<(), AppError> {
    let app = app::app().await?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("🚀 {} service running at {:#?}", cargo_pkg_name, addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}
