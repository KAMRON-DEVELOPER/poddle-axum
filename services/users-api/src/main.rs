pub mod app;
pub mod features;
pub mod services;
pub mod utilities;

use std::net::SocketAddr;
use std::result::Result::Ok;

use shared::utilities::config::Config;

use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    shared::utilities::load_service_env::load_service_env();
    let config = Config::init().await?;
    shared::utilities::observability::init_observability(&config);

    let app = app::app(&config).await?;

    info!(
        "🚀 {} service running at {:#?}",
        config.cargo_pkg_name, config.server_address
    );
    let listener = tokio::net::TcpListener::bind(config.server_address)
        .await
        .expect("msg");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();

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

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
