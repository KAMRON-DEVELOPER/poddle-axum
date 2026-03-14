pub mod handlers;
pub mod implementations;
pub mod models;
pub mod repository;
pub mod schemas;
pub mod helpers;

use crate::utilities::app_state::AppState;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};

pub fn get_routes() -> ApiRouter<AppState> {
    ApiRouter::new()
        .api_route(
            "/api/v1/users/profile",
            get(handlers::get_user_handler)
                .patch(handlers::update_user_handler)
                .delete(handlers::delete_user_handler),
        )
        .api_route(
            "/api/v1/users/auth/refresh",
            post(handlers::refresh_handler),
        )
        .api_route("/api/v1/users/auth/logout", post(handlers::logout_handler))
        .api_route("/api/v1/users/auth/verify", get(handlers::verify_handler))
        .api_route(
            "/api/v1/users/auth/google",
            get(handlers::google_oauth_handler),
        )
        .api_route(
            "/api/v1/users/auth/github",
            get(handlers::github_oauth_handler),
        )
        .api_route(
            "/api/v1/users/auth/email",
            post(handlers::continue_with_email_handler),
        )
        .api_route(
            "/api/v1/users/auth/google/callback",
            get(handlers::google_oauth_callback_handler),
        )
        .api_route(
            "/api/v1/users/auth/github/callback",
            get(handlers::github_oauth_callback_handler),
        )
        .api_route("/api/v1/users/stats", get(handlers::get_stats_handler))
        .api_route(
            "/api/v1/users/feedback",
            get(handlers::get_feedbacks_handler).post(handlers::create_feedback_handler),
        )
}
