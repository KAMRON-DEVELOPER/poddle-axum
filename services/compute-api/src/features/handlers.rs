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
    services::{
        amqp::{self, Amqp},
        redis::Redis,
    },
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
    State(amqp): State<Amqp>,
    State(config): State<Config>,
    Json(req): Json<CreateDeploymentRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!("req is {:#?}", req);
    req.validate()?;

    let user_id = claims.sub;

    ProjectRepository::get_one_by_id(&database.pool, project_id, user_id).await?;

    let mut tx = database.pool.begin().await?;

    let deployment = DeploymentRepository::create(&mut tx, user_id, project_id, req).await?;

    let channel = amqp.channel().await?;

    channel.basic_ack(delivery_tag, options);
    channel.basic_get(queue, options);
    channel.basic_nack(delivery_tag, options);
    channel.basic_publish(exchange, routing_key, options, payload, properties);
    channel.basic_qos(prefetch_count, options);
    channel.basic_recover_async(options);

    channel.channel_flow(options);

    channel.close(reply_code, reply_text);

    channel.exchange_bind(destination, source, routing_key, options, arguments);
    channel.exchange_declare(exchange, kind, options, arguments);
    channel.exchange_delete(exchange, options);
    channel.exchange_unbind(destination, source, routing_key, options, arguments);

    channel.queue_bind(queue, exchange, routing_key, options, arguments);
    channel.queue_declare(queue, options, arguments);
    channel.queue_delete(queue, options);
    channel.queue_purge(queue, options);
    channel.queue_unbind(queue, exchange, routing_key, arguments);

    channel.id();
    channel.status();

    channel.tx_commit();
    channel.tx_rollback();
    channel.tx_select();

    channel.wait_for_confirms();
    channel.wait_for_recovery(error);

    let message = serde_json::json!({
        "deployment_id": deployment.id,
        "user_id": user_id,
        "action": "create"
    });

    amqp.publish("compute.provision", &message).await?;

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
