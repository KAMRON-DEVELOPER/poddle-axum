use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::{Stream, StreamExt};
use http::{HeaderName, HeaderValue};
use std::convert::Infallible;
use url::Url;
use users_core::jwt::Claims;

use compute_core::channel_names::ChannelNames;
use factory::factories::{database::Database, redis::Redis};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    features::{
        repository::{DeploymentRepository, ProjectRepository},
        schemas::{LogResponse, LokiTailResponse},
    },
};

#[tracing::instrument(name = "stream_metrics_see_handler", skip_all, fields(project_id = %project_id), err)]
pub async fn stream_metrics_see_handler(
    Path(project_id): Path<Uuid>,
    State(redis): State<Redis>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let metrics_channel_name = ChannelNames::project_metrics(&project_id.to_string());
    let status_channel_name = ChannelNames::project_metrics(&project_id.to_string());
    let channel_name = [metrics_channel_name, status_channel_name];

    let mut pubsub = redis.pubsub().await.map_err(|err| {
        error!("Failed to connect to Redis PubSub: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    pubsub.subscribe(&channel_name).await.map_err(|err| {
        error!("Failed to subscribe to channel pattern: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let stream = pubsub.into_on_message().map(move |msg| {
        let channel = msg.get_channel_name();
        let payload: String = msg.get_payload().unwrap_or_default();
        info!(channel = %channel, payload = %payload, "pubsub payload received");

        // Ok(Event::default().event(channel).data(payload))
        Ok(Event::default().event("compute").data(payload))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[tracing::instrument(
    name = "stream_logs_see_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
        deployment_id = %deployment_id,
    ),
    err
)]
pub async fn stream_logs_see_handler(
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(cfg): State<Config>,
    State(db): State<Database>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let _exists = ProjectRepository::get_one_by_id(&claims.sub, &project_id, &db.pool)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let preset_id = DeploymentRepository::get_prest_id(&claims.sub, &deployment_id, &db.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Parse Base URL
    let mut url = Url::parse(&cfg.loki.url).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Switch Scheme safely
    match url.scheme() {
        "https" => url
            .set_scheme("wss")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        "http" => url
            .set_scheme("ws")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        _ => {}
    };

    // Set Path
    url.set_path("/loki/api/v1/tail");

    let query = format!(
        r#"{{project_id="{}", deployment_id="{}"}}"#,
        project_id, deployment_id
    );

    // Set Query Params directly on the URL object
    // This handles encoding automatically
    url.query_pairs_mut().append_pair("query", &query);

    // Build Request
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let name = HeaderName::from_static("x-scope-orgid");
    let value = HeaderValue::from_str(&preset_id.to_string())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    request.headers_mut().insert(name, value);

    let (ws_stream, _) = connect_async(request).await.map_err(|e| {
        tracing::error!("Failed to connect to Loki: {}", e);
        StatusCode::BAD_GATEWAY
    })?;
    let (_, mut stream) = ws_stream.split();

    // Create the SSE Stream
    let stream = async_stream::stream! {
        // Send an initial "connected" event
        yield Ok(Event::default().event("status").data("connected"));

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(loki_push) = serde_json::from_str::<LokiTailResponse>(&text) {
                        let log_batch = LogResponse::from(loki_push);

                        // Yield each entry individually to the frontend
                        for entry in log_batch.entries {
                            if let Ok(json) = serde_json::to_string(&entry) {
                                yield Ok(Event::default().data(json));
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                _ => {} // Ignore Ping/Pong/Binary
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
