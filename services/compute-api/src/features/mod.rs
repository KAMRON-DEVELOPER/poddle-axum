pub mod handlers;
pub mod implementations;
pub mod models;
pub mod repository;
pub mod schemas;
pub mod see;
pub mod websocket;

use crate::utilities::app_state::AppState;

use axum::{Router, routing::get};

pub fn get_routes() -> Router<AppState> {
    Router::new()
        // Projects
        .route(
            "/api/v1/projects",
            get(handlers::get_projects).post(handlers::create_project),
        )
        .route(
            "/api/v1/projects/{project_id}",
            get(handlers::get_project)
                .patch(handlers::update_project)
                .delete(handlers::delete_project),
        )
        // Deployments
        .route(
            "/api/v1/projects/{project_id}/deployments",
            get(handlers::get_deployments).post(handlers::create_deployment),
        )
        .route(
            "/api/v1/projects/{project_id}/deployments/{deployment_id}",
            get(handlers::get_deployment)
                .patch(handlers::update_deployment)
                .delete(handlers::delete_deployment),
        )
        .route(
            "/api/v1/projects/{project_id}/deployments/{deployment_id}/metrics/ws",
            get(websocket::ws_metrics),
        )
        .route(
            "/api/v1/projects/{project_id}/metrics/see",
            get(see::stream_metrics),
        )
}
