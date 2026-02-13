pub mod handlers;
pub mod implementations;
pub mod models;
pub mod queries;
pub mod repository;
pub mod schemas;
pub mod see;
pub mod webhook;
pub mod websocket;

use crate::utilities::app_state::AppState;

use axum::{Router, routing::get};

pub fn get_routes() -> Router<AppState> {
    Router::new()
        // Projects
        .route(
            "/api/v1/compute/projects",
            get(handlers::get_projects).post(handlers::create_project_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}",
            get(handlers::get_project_handler)
                .patch(handlers::update_project_handler)
                .delete(handlers::delete_project_handler),
        )
        // Deployments
        .route(
            "/api/v1/compute/projects/{project_id}/deployments",
            get(handlers::get_deployments_handler).post(handlers::create_deployment_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}",
            get(handlers::get_deployment_handler)
                .patch(handlers::update_deployment_handler)
                .delete(handlers::delete_deployment_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods",
            get(handlers::get_pods_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods/{pod_uid}/logs",
            get(handlers::get_logs_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods/{pod_uid}/logs/ws",
            get(websocket::stream_logs_ws_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods/{pod_uid}/logs/see",
            get(see::stream_logs_see_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/metrics/see",
            get(see::stream_deployment_metrics_see_handler),
        )
        .route(
            "/api/v1/compute/projects/{project_id}/metrics/see",
            get(see::stream_deployments_metrics_see_handler),
        )
        .route("/api/v1/compute/github/webhook", get(webhook::github_webhook))
}
