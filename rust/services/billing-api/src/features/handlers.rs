use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use factory::factories::database::Database;
use http_contracts::{list::schema::ListResponse, pagination::schema::Pagination};
use users_core::jwt::Claims;

use crate::{error::AppError, features::repository::BillingRepository};

pub async fn get_balance(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    let balance = BillingRepository::get_balance(user_id, &database.pool).await?;
    Ok(Json(balance))
}

pub async fn get_presets(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    let balance = BillingRepository::get_presets(user_id, &database.pool).await?;
    Ok(Json(balance))
}

pub async fn get_transactions(
    claims: Claims,
    State(database): State<Database>,
    Query(pagination): Query<Pagination>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    let (data, total) =
        BillingRepository::get_transactions(user_id, pagination, &database.pool).await?;

    Ok(Json(ListResponse { data, total }))
}
