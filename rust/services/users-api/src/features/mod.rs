pub mod handlers;
pub mod helpers;
pub mod implementations;
pub mod models;
pub mod repositories;
pub mod schemas;

use crate::utilities::app_state::AppState;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};

pub fn get_routes() -> ApiRouter<AppState> {
    ApiRouter::new()
        .api_route(
            "/api/v1/users/profile",
            get(handlers::users::get_user_handler)
                .patch(handlers::users::update_user_handler)
                .delete(handlers::users::delete_user_handler),
        )
        .api_route(
            "/api/v1/users/auth/refresh",
            post(handlers::users::refresh_handler),
        )
        .api_route(
            "/api/v1/users/auth/logout",
            post(handlers::users::logout_handler),
        )
        .api_route(
            "/api/v1/users/auth/verify",
            post(handlers::users::verify_handler),
        )
        .api_route(
            "/api/v1/users/auth/google",
            get(handlers::oauth_users::google_oauth_handler),
        )
        .api_route(
            "/api/v1/users/auth/github",
            get(handlers::oauth_users::github_oauth_handler),
        )
        .api_route(
            "/api/v1/users/auth/email",
            post(handlers::users::email_auth_handler),
        )
        .api_route(
            "/api/v1/users/auth/password-setup",
            get(handlers::oauth_users::password_setup_handler),
        )
        .api_route(
            "/api/v1/users/auth/google/callback",
            get(handlers::oauth_users::google_oauth_callback_handler),
        )
        .api_route(
            "/api/v1/users/auth/github/callback",
            get(handlers::oauth_users::github_oauth_callback_handler),
        )
        .api_route(
            "/api/v1/users/stats",
            get(handlers::stats::get_stats_handler),
        )
        .api_route(
            "/api/v1/users/feedback",
            get(handlers::feedbacks::get_feedbacks_handler)
                .post(handlers::feedbacks::create_feedback_handler),
        )
}
