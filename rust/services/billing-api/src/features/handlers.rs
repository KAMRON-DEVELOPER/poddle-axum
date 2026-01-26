use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use factory::factories::database::Database;
use http_contracts::{list::schema::ListResponse, pagination::schema::Pagination};
use users_core::jwt::Claims;

use crate::{error::AppError, features::repository::BillingRepository};

#[tracing::instrument(name = "get_balance", skip_all, fields(user_id = %claims.sub), err)]
pub async fn get_balance(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    let balance = BillingRepository::get_balance(user_id, &database.pool).await?;
    Ok(Json(balance))
}

#[tracing::instrument(name = "get_presets", skip_all, err)]
pub async fn get_presets(State(database): State<Database>) -> Result<impl IntoResponse, AppError> {
    let balance = BillingRepository::get_presets(&database.pool).await?;
    Ok(Json(balance))
}

#[tracing::instrument(name = "get_addon_price", skip_all, err)]
pub async fn get_addon_price(
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let addon_price = BillingRepository::get_addon_price(&database.pool).await?;
    Ok(Json(addon_price))
}

#[tracing::instrument(name = "get_transactions", skip_all, fields(user_id = %claims.sub), err)]
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
