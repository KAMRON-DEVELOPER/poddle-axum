use crate::features::repository::{DeploymentRepository, ProjectRepository};
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
        DeleteDeploymentMessage, DeploymentResponse, MessageResponse, UpdateDeploymentMessage,
        UpdateDeploymentRequest, UpdateProjectRequest,
    },
    services::amqp::Amqp,
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
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

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
        DeploymentRepository::create(&mut tx, user_id, project_id, req.clone()).await?;

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
    let message: CreateDeploymentMessage = (deployment.id, user_id, project_id, req).into();

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
    State(database): State<Database>,
    State(amqp): State<Amqp>,
    Json(req): Json<UpdateDeploymentRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate()?;

    let user_id: Uuid = claims.sub;

    // Update deployment in database
    let deployment =
        DeploymentRepository::update(&database.pool, deployment_id, user_id, req.clone()).await?;

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

    info!("Published scaling message for deployment {}", deployment_id);

    Ok(Json(deployment))
}

pub async fn delete_deployment(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
    State(amqp): State<Amqp>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;

    // Deleting from database
    DeploymentRepository::delete(&database.pool, deployment_id, user_id).await?;

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
        "Published deletion message for deployment {}",
        deployment_id
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse::new("Deployment deletion initiated")),
    ))
}
