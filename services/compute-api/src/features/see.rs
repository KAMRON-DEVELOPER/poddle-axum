use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    response::{Sse, sse::Event},
};
use futures::Stream;
use redis::{JsonAsyncCommands, aio::MultiplexedConnection};
use shared::{
    schemas::PodMetrics,
    services::redis::Redis,
    utilities::{config::Config, errors::AppError},
};
use uuid::Uuid;

pub async fn stream_metrics(
    Path(deployment_id): Path<Uuid>,
    State(redis): State<Redis>,
    State(config): State<Config>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let deployment_id = deployment_id.to_string();
    let mut connection = redis.connection.clone();

    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_secs(config.scrape_interval_seconds));

        loop {
            interval.tick().await;

            match get_deployment_metrics(&mut connection, &deployment_id).await {
                Ok(metrics) => {
                    let payload = serde_json::to_string(&metrics).unwrap();
                    yield Ok(Event::default().data(payload));
                }
                Err(err) => {
                    yield Ok(
                        Event::default()
                            .data(format!("error: {}", err))
                    );
                }
            }
        }
    };

    Sse::new(stream)
}

async fn get_deployment_metrics(
    connection: &mut MultiplexedConnection,
    deployment_id: &str,
) -> Result<Vec<PodMetrics>, AppError> {
    let key = format!("deployment:{}:metrics", deployment_id);

    let metrics = connection
        .json_get::<_, _, Vec<PodMetrics>>(key, "$")
        .await?;

    Ok(metrics)
}
