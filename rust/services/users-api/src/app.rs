use std::sync::Arc;

use super::config::Config;
use crate::error::AppError;
use axum::{
    Extension, Json, Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, header},
};
use http_common::{
    router::base_routes,
    trace_layer::{custom_make_span::CustomMakeSpan, custom_on_response::CustomOnResponse},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{features, utilities::app_state::AppState};

use aide::{
    axum::{ApiRouter, IntoApiResponse, routing::get},
    openapi::{Info, OpenApi},
    redoc::Redoc,
    scalar::Scalar,
    swagger::Swagger,
};

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

    let tracer_layer = TraceLayer::new_for_http()
        .make_span_with(CustomMakeSpan)
        .on_response(CustomOnResponse)
        .on_request(());

    let mut api = OpenApi {
        info: Info {
            title: cargo_pkg_name.to_string(),
            version: cargo_pkg_version.to_string(),
            description: Some(format!("{} API", cargo_pkg_name)),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let app = ApiRouter::new()
        .merge(features::get_routes())
        .merge(base_routes(cargo_pkg_name, cargo_pkg_version))
        .route("/docs/scalar", Scalar::new("/api.json").axum_route())
        .route("/docs/redoc", Redoc::new("/api.json").axum_route())
        .route("/docs/swagger", Swagger::new("/api.json").axum_route())
        .route("/api.json", get(serve_api))
        .finish_api(&mut api)
        .layer(Extension(Arc::new(api)))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .with_state(app_state)
        .layer(tracer_layer)
        .layer(cors);

    Ok(app)
}

async fn serve_api(Extension(api): Extension<Arc<OpenApi>>) -> impl IntoApiResponse {
    Json((*api).clone())
}
