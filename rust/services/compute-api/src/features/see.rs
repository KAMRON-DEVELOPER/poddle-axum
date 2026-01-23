use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::Stream;
use std::convert::Infallible;
use tokio_stream::StreamExt;

use compute_core::channel_names::ChannelNames;
use factory::factories::redis::Redis;
use tracing::{error, info};
use uuid::Uuid;

pub async fn stream_metrics(
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
