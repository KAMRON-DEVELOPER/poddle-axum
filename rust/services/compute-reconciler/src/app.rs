use crate::error::AppError;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, header},
};
use http_common::router::base_routes;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub async fn app(
    cargo_pkg_name: &'static str,
    cargo_pkg_version: &'static str,
) -> Result<Router, AppError> {
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

    let tracer_layer = TraceLayer::new_for_http();

    let app = axum::Router::new()
        .merge(base_routes(cargo_pkg_name, cargo_pkg_version))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(tracer_layer)
        .layer(cors);

    Ok(app)
}
