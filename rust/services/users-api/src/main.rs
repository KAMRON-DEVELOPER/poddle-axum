pub mod app;
pub mod config;
pub mod error;
pub mod features;
pub mod implementations;
pub mod services;
pub mod utilities;

use std::path::PathBuf;
use std::result::Result::Ok;
use std::{env, net::SocketAddr};

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

    let app = app::app(cargo_pkg_name, cargo_pkg_version, &cfg).await?;

    let listener = tokio::net::TcpListener::bind(cfg.server_address).await?;

    info!(
        "🚀 {} service running at {:#?}",
        cargo_pkg_name, cfg.server_address
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    println!("👋 Shutting down gracefully...");

    Ok(())
}
