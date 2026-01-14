use std::net::SocketAddr;

use crate::error::AppError;
use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit},
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub async fn app() -> Result<Router, AppError> {
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
        .merge(get_health_check_handler())
        .fallback(not_found_handler)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(tracing_layer)
        .layer(cors);

    Ok(app)
}

async fn not_found_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    println!("Client with {} connected", addr);
    (StatusCode::NOT_FOUND, "nothing to see here")
}

fn get_health_check_handler() -> Router {
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

    health_route
}
