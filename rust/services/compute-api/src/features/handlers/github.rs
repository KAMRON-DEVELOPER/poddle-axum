use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use compute_core::github_app::GithubApp;
use factory::factories::database::Database;
use http_contracts::{list::schema::ListResponse, message::MessageResponse};
use reqwest::Client;
use tracing::debug;
use users_core::jwt::Claims;

use crate::{error::AppError, features::schemas::CallbackParams};

#[tracing::instrument(name = "github_setup_handler", skip_all, fields(user_id = %claims.sub), err)]
pub async fn github_setup_handler(
    claims: Claims,
    State(db): State<Database>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;

    debug!(
        "installation_id: {}, setup_action: {:?}",
        params.installation_id, params.setup_action
    );

    sqlx::query!(
        r#"
        INSERT INTO installations 
            (user_id, installation_id)
        VALUES ($1, $2)
        ON CONFLICT (user_id, installation_id) DO NOTHING
        "#,
        user_id,
        params.installation_id,
    )
    .execute(&db.pool)
    .await?;

    Ok(Json(MessageResponse {
        message: "Github connected".into(),
    }))
}

#[tracing::instrument(name = "get_repositories_handler", skip_all, fields(user_id = %claims.sub), err)]
pub async fn get_repositories_handler(
    claims: Claims,
    State(github_app): State<GithubApp>,
    State(http): State<Client>,
    State(db): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;

    let installation_id = sqlx::query_scalar!(
        "SELECT installation_id FROM installations WHERE user_id = $1",
        user_id
    )
    .fetch_optional(&db.pool)
    .await?;

    let installation_id = match installation_id {
        Some(id) => id,
        None => return Err(AppError::NotFoundError("installation_id not found".into())),
    };

    let access_token = github_app
        .create_installation_token(installation_id, &http)
        .await
        .map_err(|e| AppError::InternalServerError(format!("github access token: {}", e)))?;

    let (data, total) = github_app
        .list_installation_repos(&access_token, &http)
        .await
        .map_err(|e| AppError::InternalServerError(format!("github repos: {}", e)))?;

    Ok(Json(ListResponse { data, total }))
}
