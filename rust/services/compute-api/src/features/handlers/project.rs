use crate::{error::AppError, features::repository::ProjectRepository};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use compute_core::schemas::{CreateProjectRequest, UpdateProjectRequest};
use factory::factories::database::Database;
use http_contracts::{
    list::schema::ListResponse, message::MessageResponse, pagination::schema::Pagination,
};

use users_core::jwt::Claims;
use uuid::Uuid;
use validator::Validate;

#[tracing::instrument(
    name = "get_projects",
    skip_all,
    fields(
        user_id = %claims.sub,
    ),
    err
)]
pub async fn get_projects(
    claims: Claims,
    Query(p): Query<Pagination>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let (data, total) = ProjectRepository::get_many(&user_id, &p, &database.pool).await?;

    Ok(Json(ListResponse { data, total }))
}

#[tracing::instrument(
    name = "get_project_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id
    ),
    err
)]
pub async fn get_project_handler(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let project = ProjectRepository::get_one_by_id(&user_id, &project_id, &database.pool).await?;

    Ok(Json(project))
}

#[tracing::instrument(
    name = "create_project_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
    ),
    err
)]
pub async fn create_project_handler(
    claims: Claims,
    State(database): State<Database>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    let project = ProjectRepository::create(&user_id, req, &database.pool).await?;

    Ok((StatusCode::CREATED, Json(project)))
}

#[tracing::instrument(
    name = "update_project_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id
    ),
    err
)]
pub async fn update_project_handler(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    let project = ProjectRepository::update(
        &user_id,
        &project_id,
        req.name.as_deref(),
        req.description.as_deref(),
        &database.pool,
    )
    .await?;

    Ok(Json(project))
}

#[tracing::instrument(
    name = "delete_project_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id
    )
    err
)]
pub async fn delete_project_handler(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    ProjectRepository::delete(&user_id, &project_id, &database.pool).await?;

    Ok((
        StatusCode::OK,
        Json(MessageResponse::new("Project deleted successfully")),
    ))
}
