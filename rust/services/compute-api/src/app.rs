use std::net::SocketAddr;

use super::config::Config;
use axum::{
    Router,
    extract::{ConnectInfo, DefaultBodyLimit},
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    response::IntoResponse,
};
use crate::error::AppError;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{features, utilities::app_state::AppState};

pub async fn app(config: &Config) -> Result<Router, AppError> {
    let app_state = AppState::init(&config).await?;

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
        .fallback(not_found_handler)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .with_state(app_state)
        .layer(tracing_layer)
        .layer(cors);

    Ok(app)
}

async fn not_found_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    println!("Client with {} connected", addr);
    (StatusCode::NOT_FOUND, "nothing to see here")
}
