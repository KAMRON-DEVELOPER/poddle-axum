pub mod app;
pub mod config;
pub mod error;
pub mod implementations;
pub mod services;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::result::Result::Ok;

use config::Config;
use factory::factories::observability::Observability;

use tracing::info;
use utility::shutdown_signal::shutdown_signal;

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

    let app = app::app().await?;
    let listener = tokio::net::TcpListener::bind(config.server_address).await?;

    info!(
        "🚀 {} service running at {:#?}",
        cargo_pkg_name, config.server_address
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}
