pub mod handlers;
pub mod implementations;
pub mod models;
pub mod repository;
pub mod schemas;

use crate::utilities::app_state::AppState;

use aide::axum::{ApiRouter, routing::get};

pub fn get_routes() -> ApiRouter<AppState> {
    ApiRouter::new()
        .api_route("/api/v1/billing/balance", get(handlers::get_balance))
        .api_route("/api/v1/billing/presets", get(handlers::get_presets))
        .api_route(
            "/api/v1/billing/presets/{preset_id}",
            get(handlers::get_preset),
        )
        .api_route(
            "/api/v1/billing/addon-price",
            get(handlers::get_addon_price),
        )
        .api_route(
            "/api/v1/billing/transactions",
            get(handlers::get_transactions),
        )
        .api_route("/api/v1/billing/fund", get(handlers::get_transactions))
        .api_route("/api/v1/billing/usage", get(handlers::get_transactions))
}
