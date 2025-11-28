pub mod services;
pub mod utilities;

use std::net::SocketAddr;

use crate::{
    services::prometheus::Prometheus,
    utilities::deployment_status_syncer::deployment_status_syncer,
    utilities::metrics_scraper::metrics_scraper,
};
use axum::{extract::DefaultBodyLimit, http};
use shared::{
    services::{database::Database, kubernetes::Kubernetes, redis::Redis},
    utilities::{config::Config, errors::AppError},
};
use time::macros::format_description;
use tokio::signal;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::info;
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
    let filter =
        EnvFilter::new("compute-syncer=debug,shared=debug,tower_http=warn,hyper=warn,reqwest=warn");
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

    info!("🚀 Starting compute-syncer");

    // Initialize services
    // let rustls_config = build_rustls_config(&config)?;
    let database = Database::new(&config).await?;
    let redis = Redis::new(&config).await?;
    let kubernetes = Kubernetes::new(&config).await?;
    // let amqp = Amqp::new(&config).await?;
    // let kafka = Kafka::new(&config, "compute-service-group")?;
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let prometheus = Prometheus::new(&config, http_client.clone()).await?;

    // Spawn background tasks
    info!("📊 Starting deployment status syncer");
    let status_handle = tokio::spawn(deployment_status_syncer(
        database.pool.clone(),
        kubernetes.client.clone(),
        redis.connection.clone(),
    ));

    info!("📈 Starting metrics scraper");
    let metrics_handle = tokio::spawn(metrics_scraper(
        kubernetes.client,
        prometheus.client,
        redis.connection,
    ));

    // Start HTTP server for health checks
    let _server_handle = tokio::spawn(start_health_server());

    info!("✅ All background tasks started");

    // Wait for shutdown signal
    tokio::select! {
        _ = shutdown_signal() => {
            info!("🛑 Shutdown signal received");
        }
        result = status_handle => {
            match result {
                Ok(Ok(())) => info!("Status syncer completed"),
                Ok(Err(e)) => info!("Status syncer error: {}", e),
                Err(e) => info!("Status syncer panicked: {}", e),
            }
        }
        result = metrics_handle => {
            match result {
                Ok(Ok(())) => info!("Metrics scraper completed"),
                Ok(Err(e)) => info!("Metrics scraper error: {}", e),
                Err(e) => info!("Metrics scraper panicked: {}", e),
            }
        }
    }

    info!("👋 Compute-syncer shutting down");

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
                    "service": "compute-syncer"
                }))
            }),
        )
        .route(
            "/ready",
            get(|| async {
                Json(json!({
                    "status": "ready",
                    "service": "compute-syncer"
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

    let addr = "0.0.0.0:8006";
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
