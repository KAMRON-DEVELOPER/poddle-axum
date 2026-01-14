pub mod handlers;
pub mod implementations;
pub mod models;
pub mod repository;
pub mod schemas;

use crate::utilities::app_state::AppState;

use axum::{
    Router,
    routing::{get, post},
};

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/profile",
            get(handlers::get_user_handler)
                .patch(handlers::update_user_handler)
                .delete(handlers::delete_user_handler),
        )
        .route("/api/v1/auth/refresh", post(handlers::refresh_handler))
        .route("/api/v1/auth/logout", get(handlers::logout_handler))
        .route("/api/v1/auth/verify", get(handlers::verify_handler))
        .route("/api/v1/auth/google", get(handlers::google_oauth_handler))
        .route("/api/v1/auth/github", get(handlers::github_oauth_handler))
        .route(
            "/api/v1/auth/email",
            post(handlers::continue_with_email_handler),
        )
        .route(
            "/api/v1/auth/google/callback",
            get(handlers::google_oauth_callback_handler),
        )
        .route(
            "/api/v1/auth/github/callback",
            get(handlers::github_oauth_callback_handler),
        )
}
