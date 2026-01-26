use super::config::Config;
use crate::error::AppError;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, header},
};
use http_common::router::base_routes;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{features, utilities::app_state::AppState};

pub async fn app(
    cargo_pkg_name: &'static str,
    cargo_pkg_version: &'static str,
    cfg: &Config,
) -> Result<Router, AppError> {
    let app_state = AppState::init(&cfg).await?;

    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://127.0.0.1:3000"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_credentials(true)
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            HeaderName::from_static("x-requested-with"),
        ]);

    let tracing_layer = TraceLayer::new_for_http();

    let app = axum::Router::new()
        .merge(features::get_routes())
        .merge(base_routes(cargo_pkg_name, cargo_pkg_version))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .with_state(app_state)
        .layer(tracing_layer)
        .layer(cors);

    Ok(app)
}
