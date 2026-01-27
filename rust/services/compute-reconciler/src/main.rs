pub mod app;
pub mod config;
pub mod error;
pub mod implementations;
pub mod services;
pub mod utilities;

use std::path::PathBuf;
use std::result::Result::Ok;
use std::{env, net::SocketAddr};

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
    let cargo_crate_name = env!("CARGO_CRATE_NAME");
    let cargo_pkg_name = env!("CARGO_PKG_NAME");
    let cargo_pkg_version = env!("CARGO_PKG_VERSION");

    let env_path = cargo_manifest_dir.join(".env");

    // Load service-specific .env
    dotenvy::from_path(&env_path).ok();
    // Load workspace root .env as fallback
    dotenvy::dotenv().ok();

    let path = env::var("CONFIG").unwrap_or("config.json".to_string());
    let full_path = cargo_manifest_dir.join(path);
    let cfg = Config::init(full_path).await?;

    let _guard = Observability::init(
        cargo_crate_name.to_string(),
        cargo_pkg_version.to_string(),
        &cfg.observability,
    )
    .await;

    let kubernetes = Kubernetes::new().await?;
    let database = Database::new(&cfg.database).await;
    let redis = Redis::new(&cfg.redis).await;
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let prometheus = Prometheus::new(&cfg.prometheus.url, http_client.clone()).await?;

    let mut set = JoinSet::new();

    // Spawn tasks into the set
    set.spawn(start_deployment_status_syncer(
        database.pool.clone(),
        redis.connection.clone(),
        kubernetes.client.clone(),
    ));
    set.spawn(start_metrics_scraper(cfg.clone(), redis, prometheus.client));
    set.spawn(start_reconciliation_loop(
        database.pool.clone(),
        kubernetes.client.clone(),
    ));
    set.spawn(start_health_server(
        cargo_pkg_name,
        cargo_pkg_version,
        cfg.server_address,
    ));

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

    println!("👋 Shutting down gracefully...");

    Ok(())
}

// Start a simple HTTP server for health checks and metrics
async fn start_health_server(
    cargo_pkg_name: &'static str,
    cargo_pkg_version: &'static str,
    addr: SocketAddr,
) -> Result<(), AppError> {
    let app = app::app(cargo_pkg_name, cargo_pkg_version).await?;
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
