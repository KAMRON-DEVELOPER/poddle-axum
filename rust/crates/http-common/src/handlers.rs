use std::net::SocketAddr;

use axum::{Json, extract::ConnectInfo, http::StatusCode, response::IntoResponse};
use serde_json::json;
use tracing::{info, instrument};

#[instrument(name = "root_handler", skip_all)]
pub async fn root_handler(
    cargo_pkg_name: &'static str,
    cargo_pkg_version: &'static str,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    info!(%addr, "client connected");
    Json(json!({
        "service": cargo_pkg_name,
        "version": cargo_pkg_version,
        "status": "ok"
    }))
}

#[instrument(name = "health_handler", skip_all)]
pub async fn health_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    info!(%addr, "client connected");
    Json(json!({ "status": "healthy" }))
}

#[instrument(name = "ready_handler", skip_all)]
pub async fn ready_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    info!(%addr, "client connected");
    Json(json!({ "status": "ready" }))
}

#[tracing::instrument("not_found_handler", skip_all, fields(addr = %addr))]
pub async fn not_found_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    info!(%addr, "client connected");
    (StatusCode::NOT_FOUND, "nothing to see here")
}
