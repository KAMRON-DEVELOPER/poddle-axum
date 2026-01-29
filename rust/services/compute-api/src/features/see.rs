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
use url::Url;
use users_core::jwt::Claims;

use compute_core::channel_names::ChannelNames;
use factory::factories::{database::Database, redis::Redis};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    features::{
        repository::ProjectRepository,
        schemas::{LogEntry, LokiResponse},
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

    let query = format!(
        r#"{{project_id="{}", deployment_id="{}", managed_by="poddle"}}"#,
        project_id, deployment_id
    );

    let base_url = Url::parse(&cfg.loki.url).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ws_scheme = if base_url.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    let url = format!(
        "{}://{}/loki/api/v1/tail?query={}",
        ws_scheme,
        base_url.host_str().unwrap_or("localhost"),
        urlencoding::encode(&query)
    );

    // Connect to Loki via WebSocket
    let (ws_stream, _) = connect_async(url).await.map_err(|e| {
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
                    // PARSE the Loki WebSocket JSON frame
                    if let Ok(loki_push) = serde_json::from_str::<LokiResponse>(&text) {
                        for stream in loki_push.data.result {
                            let level = stream.stream.get("level").cloned();
                            for value in stream.values {
                                // Create our clean structure
                                let entry = LogEntry {
                                    timestamp: value[0].clone(),
                                    message: value[1].clone(), // Actual log line
                                    level: level.clone(),
                                };

                                // Send as JSON event
                                if let Ok(json) = serde_json::to_string(&entry) {
                                    yield Ok(Event::default().data(json));
                                }
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
