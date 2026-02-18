use crate::{
    config::Config,
    error::AppError,
    features::{
        queries::{DeploymentMetricsQuery, LogQuery},
        repository::DeploymentRepository,
        schemas::{LogResponse, LokiResponse},
    },
    services::cache_service::CacheService,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use factory::factories::{database::Database, redis::Redis};
use http_contracts::{list::schema::ListResponse, pagination::schema::Pagination};

use reqwest::Client;
use tracing::{error, info};
use url::Url;
use users_core::jwt::Claims;
use uuid::Uuid;

#[tracing::instrument(
    name = "get_pods_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
        deployment_id = %deployment_id
    ),
    err
)]
pub async fn get_pods_handler(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    Query(p): Query<Pagination>,
    Query(q): Query<DeploymentMetricsQuery>,
    State(cfg): State<Config>,
    // State(database): State<Database>,
    State(mut redis): State<Redis>,
) -> Result<impl IntoResponse, AppError> {
    let count = q.snapshot_count(cfg.prometheus.scrape_interval_secs);

    // TODO We may add project, deployment owner checking logic later

    let (data, total) =
        CacheService::get_pods(&deployment_id.to_string(), count, &p, &mut redis.con).await?;

    Ok(Json(ListResponse { data, total }))
}

#[tracing::instrument(
    name = "get_logs_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
        deployment_id = %deployment_id,
        pod_uid = %pod_uid,
    ),
    err
)]
pub async fn get_logs_handler(
    claims: Claims,
    Path((project_id, deployment_id, pod_uid)): Path<(Uuid, Uuid, String)>,
    Query(q): Query<LogQuery>,
    State(http): State<Client>,
    State(cfg): State<Config>,
    State(db): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let preset_id =
        DeploymentRepository::get_prest_id(&claims.sub, &deployment_id, &db.pool).await?;

    // Parse Base URL
    let mut url = Url::parse(&cfg.loki.url).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set Path
    // This turns "https://loki.poddle.uz/" into "https://loki.poddle.uz/loki/api/v1/query_range"
    url.set_path("/loki/api/v1/query_range");

    let query = format!(
        r#"{{project_id="{}", deployment_id="{}"}} | pod_uid = "{}""#,
        project_id, deployment_id, pod_uid
    );

    // Convert to nanoseconds - validation happens inside
    let (start_nanos, end_nanos) = q.resolve_nanos()?;

    let query = [
        ("query", query),
        ("start", start_nanos),
        ("end", end_nanos),
        ("direction", "forward".to_string()),
        ("limit", "5000".to_string()),
    ];

    info!(
        "Sending request to Loki: {} with Tenant: {}",
        &url, preset_id
    );

    let response = http
        .get(url)
        .header("X-Scope-OrgID", &preset_id.to_string())
        .query(&query)
        .send()
        .await?;

    // Check status before parsing
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        error!("Loki Error: {}", error_text);
        return Err(StatusCode::BAD_GATEWAY.into());
    }

    let loki_response = response.json::<LokiResponse>().await?;
    let response: LogResponse = loki_response.into();

    Ok(Json(response))
}
