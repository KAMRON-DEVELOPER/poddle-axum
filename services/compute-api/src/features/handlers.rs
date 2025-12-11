use crate::features::repository::{CacheRepository, DeploymentRepository, ProjectRepository};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use lapin::{
    BasicProperties,
    options::{BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
};
use shared::{
    schemas::{
        CreateDeploymentMessage, CreateDeploymentRequest, CreateProjectRequest,
        DeleteDeploymentMessage, DeploymentResponse, MessageResponse, ProjectPageQuery,
        UpdateDeploymentMessage, UpdateDeploymentRequest, UpdateProjectRequest,
    },
    services::{amqp::Amqp, redis::Redis},
    utilities::config::Config,
};
use shared::{
    schemas::{ListResponse, Pagination},
    services::database::Database,
    utilities::{errors::AppError, jwt::Claims},
};
use tracing::{debug, info};
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

    let project = ProjectRepository::create(&database.pool, user_id, req).await?;

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
    Query(ProjectPageQuery { minutes }): Query<ProjectPageQuery>,
    State(database): State<Database>,
    State(mut redis): State<Redis>,
    State(config): State<Config>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;
    let points_count = minutes * 60 / config.scrape_interval_seconds;

    let (total, deployments) =
        DeploymentRepository::get_all_by_project(user_id, project_id, &database.pool).await?;

    if total == 0 {
        return Ok(Json(ListResponse {
            data: vec![],
            total: 0,
        }));
    }

    // if deployments.is_empty() {
    //     return Ok(Json(ListResponse {
    //         data: vec![],
    //         total: 0,
    //     }));
    // }

    let deployment_ids: Vec<String> = deployments.iter().map(|d| d.id.to_string()).collect();
    let deployment_ids: Vec<&str> = deployment_ids.iter().map(|s| s.as_str()).collect();
    let deployment_metrics = CacheRepository::get_deployment_metrics(
        points_count,
        deployment_ids,
        &mut redis.connection,
    )
    .await?;

    let data: Vec<DeploymentResponse> = deployments
        .into_iter()
        .zip(deployment_metrics.into_iter())
        .map(|pair| pair.into())
        .collect();

    Ok(Json(ListResponse { data, total }))
}

pub async fn get_deployment(
    claims: Claims,
    Path((_project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let deployment =
        DeploymentRepository::get_by_id(&database.pool, user_id, deployment_id).await?;

    Ok(Json(deployment))
}

pub async fn create_deployment(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    State(database): State<Database>,
    State(amqp): State<Amqp>,
    Json(req): Json<CreateDeploymentRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!("req is {:#?}", req);
    req.validate()?;

    let user_id = claims.sub;

    // Verify project exists
    ProjectRepository::get_one_by_id(&database.pool, project_id, user_id).await?;

    // Start database transaction
    let mut tx = database.pool.begin().await?;

    // Create deployment record
    let deployment =
        DeploymentRepository::create(user_id, project_id, req.clone(), &mut tx).await?;

    // Get RabbitMQ channel
    let channel = amqp.channel().await?;

    // Declare exchange (idempotent)
    channel
        .exchange_declare(
            "compute",
            lapin::ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                auto_delete: false,
                internal: false,
                nowait: false,
                passive: false,
            },
            FieldTable::default(),
        )
        .await?;

    // Declare queue for provisioner
    channel
        .queue_declare(
            "compute.create",
            QueueDeclareOptions {
                durable: true,
                exclusive: false,
                auto_delete: false,
                nowait: false,
                passive: false,
            },
            FieldTable::default(),
        )
        .await?;

    // Bind queue to exchange
    channel
        .queue_bind(
            "compute.create",
            "compute",
            "compute.create",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Prepare message
    let message: CreateDeploymentMessage = (user_id, project_id, deployment.id, req).into();

    let payload = serde_json::to_vec(&message)?;

    // Publish message
    channel
        .basic_publish(
            "compute",
            "compute.create",
            BasicPublishOptions {
                mandatory: false,
                immediate: false,
            },
            &payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_content_type("application/json".into()),
        )
        .await?
        .await?;

    info!(
        "Published deployment creation message for {}",
        deployment.id
    );

    // Commit transaction
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(deployment)))
}

pub async fn update_deployment(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(amqp): State<Amqp>,
    State(database): State<Database>,
    Json(req): Json<UpdateDeploymentRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    // Start database transaction
    let mut tx = database.pool.begin().await?;

    // Update deployment in database
    let deployment =
        DeploymentRepository::update(user_id, deployment_id, req.clone(), &mut tx).await?;

    // Get RabbitMQ channel
    let channel = amqp.channel().await?;

    // Prepare message
    let message: UpdateDeploymentMessage = (user_id, project_id, deployment_id, req).into();

    let payload = serde_json::to_vec(&message)?;

    // Publish message
    channel
        .basic_publish(
            "compute",
            "compute.update",
            BasicPublishOptions::default(),
            &payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_content_type("application/json".into()),
        )
        .await?
        .await?;

    info!("Published deployment update message for {}", deployment_id);

    // Commit transaction
    tx.commit().await?;

    Ok(Json(deployment))
}

pub async fn delete_deployment(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
    State(amqp): State<Amqp>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;

    // Start database transaction
    let mut tx = database.pool.begin().await?;

    // Deleting from database
    DeploymentRepository::delete(user_id, deployment_id, &mut tx).await?;

    // Get RabbitMQ channel
    let channel = amqp.channel().await?;

    // Prepare message
    let message = DeleteDeploymentMessage {
        deployment_id,
        user_id,
        project_id,
        timestamp: chrono::Utc::now().timestamp(),
    };

    let payload = serde_json::to_vec(&message)?;

    // Publish message
    channel
        .basic_publish(
            "compute",
            "compute.delete",
            BasicPublishOptions::default(),
            &payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_content_type("application/json".into()),
        )
        .await?
        .await?;

    info!(
        "Published deployment deletion message for {}",
        deployment_id
    );

    // Commit transaction
    tx.commit().await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse::new("Deployment deletion initiated")),
    ))
}
