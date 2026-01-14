use axum::{Json, extract::State, response::IntoResponse};
use factory::factories::database::Database;
use http_contracts::list::schema::ListResponse;
use users_core::jwt::Claims;

use crate::{error::AppError, features::repository::BillingRepository};

pub async fn get_balance(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    let balance = BillingRepository::get_user_balance(&database.pool, user_id).await?;
    Ok(Json(balance))
}

pub async fn get_transactions(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;

    let transactions = BillingRepository::get_transactions(&database.pool, user_id).await?;

    Ok(Json(ListResponse {
        total: i64::try_from(transactions.len()).unwrap_or_else(|_| 0),
        data: transactions,
    }))
}
