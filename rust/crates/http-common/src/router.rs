use axum::{Router, routing::get};

use crate::handlers::{health_handler, not_found_handler, ready_handler, root_handler};

pub fn base_routes<S>(cargo_pkg_name: &'static str, cargo_pkg_version: &'static str) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let name = cargo_pkg_name;
    let version = cargo_pkg_version;

    Router::new()
        .route(
            "/",
            get(move |connect_info| root_handler(name, version, connect_info)),
        )
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .fallback(not_found_handler)
}
