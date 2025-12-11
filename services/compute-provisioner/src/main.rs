pub mod services;
pub mod utilities;

use std::net::SocketAddr;

use crate::services::{consumer::start_consumer, vault_service::VaultService};
use axum::{extract::DefaultBodyLimit, http};
use shared::{
    services::{amqp::Amqp, database::Database, kubernetes::Kubernetes, redis::Redis},
    utilities::{config::Config, errors::AppError},
};
use time::macros::format_description;
use tokio::{signal, task::JoinSet};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{error, info};
use tracing_subscriber::{
    EnvFilter, fmt::time::LocalTime, layer::SubscriberExt, util::SubscriberInitExt,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Load .env file
    match dotenvy::dotenv() {
        Ok(path) => {
            info!("Loaded .env file from {}", path.display());
        }
        Err(dotenvy::Error::Io(ref err)) if err.kind() == std::io::ErrorKind::NotFound => {
            println!(".env file not found, continuing without it");
        }
        Err(e) => {
            println!("Couldn't load .env file: {}", e);
        }
    }

    // Initialize config
    let config = Config::init().await?;

    // Initialize tracing
    let filter = EnvFilter::new(
        "compute_provisioner=debug,shared=debug,tower_http=warn,hyper=warn,reqwest=warn",
    );
    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second]"
    ));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_timer(timer),
        )
        .init();

    info!("🚀 Starting compute-provisioner");

    // Initialize services
    // let rustls_config = build_rustls_config(&config)?;
    let database = Database::new(&config).await?;
    let redis = Redis::new(&config).await?;
    let kubernetes = Kubernetes::new(&config).await?;
    let amqp = Amqp::new(&config).await?;
    // let kafka = Kafka::new(&config, "compute-service-group")?;
    // let http_client = reqwest::ClientBuilder::new()
    //     .redirect(reqwest::redirect::Policy::none())
    //     .build()?;
    let vault_service = VaultService::init(&config).await?;

    let mut set = JoinSet::new();

    // Spawn background tasks
    set.spawn(start_consumer(
        amqp,
        redis,
        database.pool,
        kubernetes.client,
        config.base_domain,
        config.enable_tls,
        config.cluster_issuer,
        vault_service,
    ));
    set.spawn(start_health_server());

    info!("✅ All background tasks started");

    // Unified shutdown logic
    tokio::select! {
        _ = shutdown_signal() => {
            info!("🛑 Shutdown signal received");
            set.shutdown().await;
        }
        // If ANY task exits (crashes or finishes), this branch runs
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

    info!("👋 Compute-provisioner shutting down");

    Ok(())
}

/// Start a simple HTTP server for health checks and metrics
async fn start_health_server() -> Result<(), AppError> {
    use axum::{Json, Router, routing::get};
    use serde_json::json;

    let health_route = Router::new()
        .route(
            "/health",
            get(|| async {
                Json(json!({
                    "status": "healthy",
                    "service": "compute-provisioner"
                }))
            }),
        )
        .route(
            "/ready",
            get(|| async {
                Json(json!({
                    "status": "ready",
                    "service": "compute-provisioner"
                }))
            }),
        );

    let tracing_layer = TraceLayer::new_for_http()
        .on_request(|request: &http::Request<_>, _span: &tracing::Span| {
            let method = request.method();
            let uri = request.uri();
            let matched_path = request
                .extensions()
                .get::<axum::extract::MatchedPath>()
                .map(|p| p.as_str())
                .unwrap_or("<unknown>");

            if uri.query().is_some() {
                info!("{} {} {}", method, matched_path, uri);
            } else {
                info!("{} {}", method, matched_path);
            }
        })
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO));

    let app = Router::new()
        .merge(health_route)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(tracing_layer);

    let addr = "0.0.0.0:8004";
    info!("🏥 Health check server running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
