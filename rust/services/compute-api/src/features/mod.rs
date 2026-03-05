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

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};

pub fn get_routes() -> ApiRouter<AppState> {
    ApiRouter::new()
        // Projects
        .api_route(
            "/api/v1/compute/projects",
            get(handlers::project::get_projects).post(handlers::project::create_project_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}",
            get(handlers::project::get_project_handler)
                .patch(handlers::project::update_project_handler)
                .delete(handlers::project::delete_project_handler),
        )
        // Deployments
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments",
            get(handlers::deployment::get_deployments_handler).post(handlers::deployment::create_deployment_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}",
            get(handlers::deployment::get_deployment_handler)
                .patch(handlers::deployment::update_deployment_handler)
                .delete(handlers::deployment::delete_deployment_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods",
            get(handlers::pod::get_pods_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods/{pod_uid}/logs",
            get(handlers::pod::get_logs_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods/{pod_uid}/logs/ws",
            get(websocket::stream_logs_ws_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/pods/{pod_uid}/logs/see",
            get(see::stream_logs_see_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/deployments/{deployment_id}/metrics/see",
            get(see::stream_deployment_metrics_see_handler),
        )
        .api_route(
            "/api/v1/compute/projects/{project_id}/metrics/see",
            get(see::stream_deployments_metrics_see_handler),
        )
        .api_route("/api/v1/compute/github/repositories", get(handlers::github::get_repositories_handler))
        .api_route("/api/v1/compute/github/setup", post(handlers::github::github_setup_handler))
        .api_route("/api/v1/compute/github/webhook", get(webhook::github_webhook))
}
