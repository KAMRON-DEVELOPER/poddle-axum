use crate::error::AppError;
use axum::{
    extract::{
        Path, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use compute_core::schemas::PodMetrics;
use factory::factories::redis::Redis;
use redis::{JsonAsyncCommands, aio::MultiplexedConnection};
use serde_json::json;
use tokio::time::{self, Duration};
use uuid::Uuid;

pub async fn ws_metrics(
    ws: WebSocketUpgrade,
    Path(deployment_id): Path<Uuid>,
    State(redis): State<Redis>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, deployment_id, redis))
}

async fn handle_socket(mut socket: WebSocket, deployment_id: Uuid, redis: Redis) {
    let mut conn = redis.connection.clone();
    let deployment_id = deployment_id.to_string();

    let mut interval = time::interval(Duration::from_secs(25));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match get_deployment_metrics(&mut conn, &deployment_id).await {
                    Ok(metrics) => {
                        let payload = serde_json::to_string(&metrics).unwrap();
                        if socket.send(axum::extract::ws::Message::Text(payload.into())).await.is_err() {
                            // Client disconnected
                            break;
                        }
                    }
                    Err(err) => {
                        let payload = json!({
                            "error": err.to_string()
                        }).to_string();

                        let _ = socket.send(axum::extract::ws::Message::Text(payload.into())).await;
                    }
                }
            }


        }
    }
}

async fn get_deployment_metrics(
    connection: &mut MultiplexedConnection,
    deployment_id: &str,
) -> Result<Vec<PodMetrics>, AppError> {
    let key = format!("deployment:{}:metrics", deployment_id);
    let metrics = connection
        .json_get::<_, _, Vec<PodMetrics>>(&key, "$")
        .await?;
    Ok(metrics)
}
