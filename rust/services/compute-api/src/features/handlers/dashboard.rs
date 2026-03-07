use crate::{error::AppError, features::repository::DashboardRepository};
use aide::axum::IntoApiResponse;
use axum::{Json, extract::State};
use factory::factories::{database::Database, redis::Redis};

use users_core::jwt::Claims;
use uuid::Uuid;

#[tracing::instrument(name = "get_dashboard_handler", skip_all, fields(user_id = %claims.sub), err)]
pub async fn get_dashboard_handler(
    claims: Claims,
    State(database): State<Database>,
    State(mut redis): State<Redis>,
) -> Result<impl IntoApiResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let res = DashboardRepository::get_dashboard(&user_id, &database.pool, &mut redis.con).await?;

    Ok(Json(res))
}
