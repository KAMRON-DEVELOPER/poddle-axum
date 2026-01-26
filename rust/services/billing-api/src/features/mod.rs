pub mod handlers;
pub mod implementations;
pub mod models;
pub mod repository;
pub mod schemas;

use crate::utilities::app_state::AppState;

use axum::{Router, routing::get};

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/billing/balance", get(handlers::get_balance))
        .route("/api/v1/billing/presets", get(handlers::get_presets))
        .route(
            "/api/v1/billing/addon-price",
            get(handlers::get_addon_price),
        )
        .route(
            "/api/v1/billing/transactions",
            get(handlers::get_transactions),
        )
        .route("/api/v1/billing/fund", get(handlers::get_transactions))
        .route("/api/v1/billing/usage", get(handlers::get_transactions))
}
