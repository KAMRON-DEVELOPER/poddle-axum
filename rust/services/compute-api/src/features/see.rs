use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::{Stream, StreamExt};
use std::convert::Infallible;
use users_core::jwt::Claims;

use compute_core::channel_names::ChannelNames;
use factory::factories::{database::Database, redis::Redis};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info};
use uuid::Uuid;

use crate::{config::Config, features::repository::ProjectRepository};

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
    name = "stream_logs_hander",
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
    // TODO, for now it is enough, later we must check project and deployment beforehand
    ProjectRepository::get_one_by_id(&claims.sub, &project_id, &db.pool)
        .await
        .ok();

    let query = format!(
        r#"{{project_id={}, deployment_id="{}", managed_by="poddle"}}"#,
        project_id, deployment_id
    );

    let ws_loki_url = if cfg.loki.url.contains("https://") {
        cfg.loki.url.replace("https://", "wss://")
    } else if cfg.loki.url.contains("http://") {
        cfg.loki.url.replace("http://", "ws://")
    } else {
        cfg.loki.url
    };

    let url = format!(
        "{}/loki/api/v1/tail?query={}",
        ws_loki_url,
        urlencoding::encode(&query)
    );

    // Connect to Loki via WebSocket
    let (ws_stream, _) = connect_async(url)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (_, mut stream) = ws_stream.split();

    // Transform the WS Stream into an SSE Stream
    let sse_stream = async_stream::stream! {
        while let Some(Ok(msg)) = stream.next().await {
            if let Message::Text(text) = msg {
                // Loki sends a JSON object containing the log lines
                yield Ok(Event::default().data(text));
            }
        }
    };

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}
