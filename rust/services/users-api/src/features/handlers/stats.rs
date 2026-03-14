use aide::axum::IntoApiResponse;
use axum::{Json, extract::State};
use factory::factories::database::Database;
use tracing::instrument;

use crate::{error::AppError, features::schemas::StatsResponse};

#[instrument(name = "get_stats_handler", skip_all)]
pub async fn get_stats_handler(
    State(database): State<Database>,
) -> Result<impl IntoApiResponse, AppError> {
    let users_total = sqlx::query_scalar!("SELECT COUNT(*) from users")
        .fetch_one(&database.pool)
        .await?;
    let deployments_total = sqlx::query!("SELECT COUNT(*) as count from deployments")
        .fetch_one(&database.pool)
        .await?;

    Ok(Json(StatsResponse {
        users_total: users_total.unwrap_or(0),
        deployments_total: deployments_total.count.unwrap_or(0),
    }))
}
