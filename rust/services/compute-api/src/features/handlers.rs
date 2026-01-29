use crate::{
    config::Config,
    error::AppError,
    features::{
        repository::{
            CacheRepository, DeploymentPresetRepository, DeploymentRepository, ProjectRepository,
        },
        schemas::{LogEntry, LogQuery, LokiResponse, ProjectPageWithPaginationQuery},
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use compute_core::schemas::{
    CreateDeploymentMessage, CreateDeploymentRequest, CreateProjectRequest,
    DeleteDeploymentMessage, DeploymentResponse, UpdateDeploymentMessage, UpdateDeploymentRequest,
    UpdateProjectRequest,
};
use factory::factories::{
    amqp::{Amqp, AmqpPropagator},
    database::Database,
    redis::Redis,
};
use http_contracts::{
    list::schema::ListResponse, message::MessageResponse, pagination::schema::Pagination,
};
use lapin::{
    BasicProperties,
    options::{BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
};

use reqwest::Client;
use tracing::{Instrument, debug, info, info_span};
use users_core::jwt::Claims;
use uuid::Uuid;
use validator::Validate;

// ============================================
// PROJECT HANDLERS
// ============================================

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
    Query(pagination): Query<Pagination>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let (data, total) = ProjectRepository::get_many(&user_id, &pagination, &database.pool).await?;

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

// ============================================
// DEPLOYMENT HANDLERS
// ============================================

#[tracing::instrument(
    name = "get_deployments_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id
    ),
    err
)]
pub async fn get_deployments_handler(
    claims: Claims,
    Path(project_id): Path<Uuid>,
    Query(ProjectPageWithPaginationQuery {
        pagination,
        project_page_query,
    }): Query<ProjectPageWithPaginationQuery>,
    // Query(pagination): Query<Pagination>,
    // Query(ProjectPageQuery { minutes }): Query<ProjectPageQuery>,
    State(config): State<Config>,
    State(database): State<Database>,
    State(mut redis): State<Redis>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;
    let points_count = project_page_query.minutes * 60 / config.prometheus.scrape_interval_seconds;

    let (deployments, total) = DeploymentRepository::get_all_by_project(
        &user_id,
        &project_id,
        &pagination,
        &database.pool,
    )
    .await?;

    if total == 0 {
        return Ok(Json(ListResponse {
            data: vec![],
            total: 0,
        }));
    }

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

#[tracing::instrument(
    name = "get_deployment_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
        deployment_id = %deployment_id
    ),
    err
)]
pub async fn get_deployment_handler(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user_id: Uuid = claims.sub;

    let deployment =
        DeploymentRepository::get_by_id(&user_id, &deployment_id, &database.pool).await?;

    Ok(Json(deployment))
}

#[tracing::instrument(
    name = "create_deployment_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
    ),
    err
)]
pub async fn create_deployment_handler(
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
    ProjectRepository::get_one_by_id(&user_id, &project_id, &database.pool).await?;

    // Start database transaction
    let mut tx = database.pool.begin().await?;

    // Create deployment record
    let deployment =
        DeploymentRepository::create(&user_id, &project_id, req.clone(), &mut tx).await?;

    // Get RabbitMQ channel
    let channel = amqp.channel().await;

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
    let preset = DeploymentPresetRepository::get_by_id(&req.preset_id, &mut *tx).await?;
    if preset.max_addon_cpu_millicores < req.addon_cpu_millicores.unwrap_or_default()
        || preset.max_addon_memory_mb < req.addon_memory_mb.unwrap_or_default()
    {
        return Err(AppError::ValidationError(format!(
            "Requested add-ons exceed limits for preset '{}'. Max CPU: {}m, Max Memory: {}MB",
            preset.name, preset.max_addon_cpu_millicores, preset.max_addon_memory_mb
        )));
    }

    let message: CreateDeploymentMessage = (user_id, project_id, deployment.id, preset, req).into();

    let payload = serde_json::to_vec(&message)?;

    let mut headers = FieldTable::default();
    AmqpPropagator::inject_context(&mut headers);

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
                .with_content_type("application/json".into())
                .with_headers(headers),
        )
        .instrument(info_span!("basic_publish.compute.create"))
        .await?
        .await?;

    info!(deployment_id = %deployment.id, "Published deployment creation message");

    // Commit transaction
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(deployment)))
}

#[tracing::instrument(
    name = "update_deployment_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
    ),
    err
)]
pub async fn update_deployment_handler(
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
        DeploymentRepository::update(&user_id, &deployment_id, req.clone(), &mut tx).await?;

    // Get RabbitMQ channel
    let channel = amqp.channel().await;

    // Prepare message
    let preset = if let Some(preset_id) = req.preset_id {
        let preset = DeploymentPresetRepository::get_by_id(&preset_id, &mut *tx).await?;
        if preset.max_addon_cpu_millicores < req.addon_cpu_millicores.unwrap_or_default()
            || preset.max_addon_memory_mb < req.addon_memory_mb.unwrap_or_default()
        {
            return Err(AppError::ValidationError(format!(
                "Requested add-ons exceed limits for preset '{}'. Max CPU: {}m, Max Memory: {}MB",
                preset.name, preset.max_addon_cpu_millicores, preset.max_addon_memory_mb
            )));
        }
        Some(preset)
    } else {
        None
    };

    let message: UpdateDeploymentMessage = (user_id, project_id, deployment_id, preset, req).into();

    let payload = serde_json::to_vec(&message)?;

    let mut headers = FieldTable::default();
    AmqpPropagator::inject_context(&mut headers);

    // Publish message
    channel
        .basic_publish(
            "compute",
            "compute.update",
            BasicPublishOptions::default(),
            &payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_content_type("application/json".into())
                .with_headers(headers),
        )
        .instrument(info_span!("basic_publish.compute.update"))
        .await?
        .await?;

    info!("Published deployment update message for {}", deployment_id);

    // Commit transaction
    tx.commit().await?;

    Ok(Json(deployment))
}

#[tracing::instrument(
    name = "delete_deployment_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
    ),
    err
)]
pub async fn delete_deployment_handler(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(database): State<Database>,
    State(amqp): State<Amqp>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;

    // Start database transaction
    let mut tx = database.pool.begin().await?;

    // Deleting from database
    DeploymentRepository::delete(&user_id, &deployment_id, &mut tx).await?;

    // Get RabbitMQ channel
    let channel = amqp.channel().await;

    // Prepare message
    let message = DeleteDeploymentMessage {
        deployment_id,
        user_id,
        project_id,
        timestamp: chrono::Utc::now().timestamp(),
    };

    let payload = serde_json::to_vec(&message)?;

    let mut headers = FieldTable::default();
    AmqpPropagator::inject_context(&mut headers);

    // Publish message
    channel
        .basic_publish(
            "compute",
            "compute.delete",
            BasicPublishOptions::default(),
            &payload,
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_content_type("application/json".into())
                .with_headers(headers),
        )
        .instrument(info_span!("basic_publish.compute.delete"))
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

#[tracing::instrument(
    name = "get_logs_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
        deployment_id = %deployment_id,
    ),
    err
)]
pub async fn get_logs_handler(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<LogQuery>,
    State(http): State<Client>,
    State(cfg): State<Config>,
    State(db): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    ProjectRepository::get_one_by_id(&claims.sub, &project_id, &db.pool)
        .await
        .ok();

    let url = format!("{}/loki/api/v1/query_range", cfg.loki.url);
    let query = format!(
        r#"{{project_id="{}", deployment_id="{}", managed_by="poddle"}}"#,
        project_id, deployment_id
    );

    let start = q
        .start_time
        .unwrap_or_else(|| (chrono::Utc::now() - chrono::Duration::minutes(15)).to_rfc3339());
    let limit = q.limit.unwrap_or_else(|| 100).to_string();

    let response = http
        .get(url)
        .query(&[
            ("query", query.as_str()),
            ("start", &start),
            ("limit", &limit),
            ("direction", "backward"),
        ])
        .send()
        .await?;

    let loki_data = response.json::<LokiResponse>().await?;

    // Flatten and Clean Data
    let mut clean_logs: Vec<LogEntry> = Vec::new();

    for stream in loki_data.data.result {
        // Extract level from labels if available
        let level = stream.stream.get("level").cloned();

        for value in stream.values {
            // value[0] is timestamp (ns), value[1] is the log line
            clean_logs.push(LogEntry {
                timestamp: value[0].clone(),
                message: value[1].clone(),
                level: level.clone(),
            });
        }
    }

    Ok(axum::Json(clean_logs))
}
