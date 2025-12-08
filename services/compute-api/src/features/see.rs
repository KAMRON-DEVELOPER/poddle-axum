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

use shared::{services::redis::Redis, utilities::channel_names::ChannelNames};
use tracing::{error, info};
use uuid::Uuid;

pub async fn stream_metrics(
    Path((_project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(redis): State<Redis>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let status_channel = ChannelNames::deployment_status(deployment_id);
    let metrics_channel = ChannelNames::deployment_metrics(deployment_id);
    let channels = [status_channel.clone(), metrics_channel.clone()];

    let mut pubsub = redis.pubsub().await.map_err(|err| {
        error!("Failed to connect to Redis PubSub: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    pubsub.subscribe(&channels).await.map_err(|err| {
        error!("Failed to subscribe to channels: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("Subscribed to channels: {:?}", channels);

    let stream = pubsub.into_on_message().map(move |msg| {
        let channel = msg.get_channel_name();
        let payload: String = msg.get_payload().unwrap_or_default();

        Ok(Event::default().event(channel).data(payload))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
