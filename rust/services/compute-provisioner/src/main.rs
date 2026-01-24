pub mod app;
pub mod config;
pub mod error;
pub mod implementations;
pub mod services;

use core::panic;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::result::Result::Ok;

use config::Config;
use factory::factories::{
    amqp::Amqp, database::Database, kubernetes::Kubernetes, observability::Observability,
    redis::Redis,
};

use tokio::task::JoinSet;
use tracing::{error, info};
use utility::shutdown_signal::shutdown_signal;

use crate::{
    error::AppError,
    services::{
        consumer::start_consumer, kubernetes_service::KubernetesService,
        vault_service::VaultService,
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
    let cargo_pkg_version = env!("CARGO_PKG_VERSION").into();

    let env_path = cargo_manifest_dir.join(".env");

    // Load service-specific .env
    dotenvy::from_path(&env_path).ok();
    // Load workspace root .env as fallback
    dotenvy::dotenv().ok();

    let cfg = Config::init(cargo_manifest_dir).await?;

    let _guard = Observability::init(
        &cfg.otel_exporter_otlp_endpoint,
        cargo_crate_name,
        cargo_pkg_version,
    )
    .await;

    // Initialize services
    // let rustls_config = build_rustls_config(&cfg)?;
    let database = Database::new(&cfg.database).await;
    let redis = Redis::new(&cfg.redis).await;
    let kubernetes = Kubernetes::new().await?;
    let amqp = Amqp::new(&cfg.amqp_addr).await;
    // let kafka = Kafka::new(&cfg, "compute-service-group")?;
    // let http_client = reqwest::ClientBuilder::new()
    //     .redirect(reqwest::redirect::Policy::none())
    //     .build()?;
    let vault_service = VaultService::init(&cfg.vault).await?;

    let kubernetes_service = KubernetesService {
        client: kubernetes.client,
        pool: database.pool,
        redis,
        amqp,
        vault_service,
        cfg: cfg.kubernetes,
    };

    kubernetes_service.init().await?;

    let mut set = JoinSet::new();

    // Spawn background tasks
    set.spawn(start_consumer(kubernetes_service));
    set.spawn(start_health_server(cargo_pkg_name, cfg.server_address));

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

    Ok(())
}

// Start a simple HTTP server for health checks and metrics
async fn start_health_server(cargo_pkg_name: &str, server_address: String) -> Result<(), AppError> {
    let app = app::app().await?;
    let addr = server_address
        .parse::<SocketAddr>()
        .expect("Server address is invalid");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("🚀 {} service running at {:#?}", cargo_pkg_name, addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    println!("👋 Shutting down gracefully...");

    Ok(())
}
