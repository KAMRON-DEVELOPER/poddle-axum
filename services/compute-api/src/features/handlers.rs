use crate::{
    features::repository::{DeploymentRepository, ProjectRepository},
    services::compute_cache::ComputeCache,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use shared::{
    schemas::{
        CreateDeploymentRequest, CreateProjectRequest, DeploymentResponse, MessageResponse,
        ScaleDeploymentRequest, UpdateProjectRequest,
    },
    services::redis::Redis,
};
use shared::{
    schemas::{ListResponse, Pagination},
    services::database::Database,
    utilities::{config::Config, errors::AppError, jwt::Claims},
};
use tracing::debug;
use uuid::Uuid;
use validator::Validate;

// ============================================
// PROJECT HANDLERS
// ============================================

pub async fn get_projects(
    claims: Claims,
    Query(pagination): Query<Pagination>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let (projects, total) =
        ProjectRepository::get_many(&database.pool, user_id, pagination).await?;

    Ok(Json(ListResponse {
        data: projects,
        total,
    }))
}

pub async fn get_project(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let project = ProjectRepository::get_one_by_id(&database.pool, project_id, user_id).await?;

    Ok(Json(project))
}

pub async fn create_project(
    claims: Claims,
    State(database): State<Database>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    let project = ProjectRepository::create(
        &database.pool,
        user_id,
        &req.name,
        req.description.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(project)))
}

pub async fn update_project(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    let project = ProjectRepository::update(
        &database.pool,
        project_id,
        user_id,
        req.name.as_deref(),
        req.description.as_deref(),
    )
    .await?;

    Ok(Json(project))
}

pub async fn delete_project(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    ProjectRepository::delete(&database.pool, project_id, user_id).await?;

    Ok((
        StatusCode::OK,
        Json(MessageResponse::new("Project deleted successfully")),
    ))
}

// ============================================
// DEPLOYMENT HANDLERS
// ============================================

pub async fn get_deployments(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(redis): State<Redis>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let mut cache = ComputeCache(redis.connection);

    let d = cache.get_deployments_state(project_id).await?;

    let deployments =
        DeploymentRepository::get_all_by_project(&database.pool, project_id, user_id).await?;

    let response: Vec<DeploymentResponse> = deployments
        .into_iter()
        .map(|d| {
            let resources = serde_json::from_value(d.resources).unwrap_or_default();
            DeploymentResponse {
                id: d.id,
                project_id: d.project_id,
                name: d.name,
                image: d.image,
                status: d.status,
                replicas: d.replicas,
                resources,
                external_url: None,
                created_at: d.created_at,
                updated_at: d.updated_at,
            }
        })
        .collect();

    Ok(Json(ListResponse {
        total: i64::try_from(response.len()).unwrap_or_else(|_| 0),
        data: response,
    }))
}

pub async fn get_deployment(
    claims: Claims,
    Path((_, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    Ok(Json(detail))
}

pub async fn create_deployment(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
    State(config): State<Config>,
    Json(req): Json<CreateDeploymentRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!("req is {:#?}", req);
    req.validate()?;

    let user_id = claims.sub;

    ProjectRepository::get_one_by_id(&database.pool, project_id, user_id).await?;

    // Start database transaction
    let mut tx = database.pool.begin().await?;

    DeploymentRepository::create(tx, user_id, project_id, req);

    // Commit transaction
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(deployment)))
}

pub async fn scale_deployment(
    claims: Claims,
    Path((_, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
    Json(req): Json<ScaleDeploymentRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    Ok(Json(deployment))
}

pub async fn delete_deployment(
    claims: Claims,
    Path((_, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    Ok((
        StatusCode::OK,
        Json(MessageResponse::new("Deployment deleted successfully")),
    ))
}
