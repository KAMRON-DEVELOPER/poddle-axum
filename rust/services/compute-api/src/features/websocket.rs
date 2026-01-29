use crate::{config::Config, error::AppError, features::repository::ProjectRepository};
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use compute_core::schemas::PodMetrics;
use factory::factories::{database::Database, redis::Redis};
use redis::{JsonAsyncCommands, aio::MultiplexedConnection};
use serde_json::json;
use tokio::time::{self, Duration};
use users_core::jwt::Claims;
use uuid::Uuid;

use futures::{sink::SinkExt, stream::StreamExt};
use tokio_tungstenite::connect_async;

pub async fn stream_metrics_ws_handler(
    ws: WebSocketUpgrade,
    Path(deployment_id): Path<Uuid>,
    State(redis): State<Redis>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_stream_metrics(deployment_id, redis, socket))
}

async fn handle_stream_metrics(deployment_id: Uuid, redis: Redis, mut socket: WebSocket) {
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

pub async fn stream_logs_ws_handler(
    ws: WebSocketUpgrade,
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(cfg): State<Config>,
    State(db): State<Database>,
) -> impl IntoResponse {
    // TODO, for now it is enough, later we must check project and deployment beforehand
    ProjectRepository::get_one_by_id(&claims.sub, &project_id, &db.pool)
        .await
        .ok();
    ws.on_upgrade(move |socket| handle_stream_logs(project_id, deployment_id, cfg, socket))
}

async fn handle_stream_logs(
    project_id: Uuid,
    deployment_id: Uuid,
    cfg: Config,
    mut client_socket: WebSocket,
) {
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

    // Connect to Loki
    let (loki_socket, _) = match connect_async(url).await {
        Ok(s) => s,
        Err(e) => {
            let _ = client_socket
                .send(Message::Text(
                    format!("Error connecting to logs: {}", e).into(),
                ))
                .await;
            return;
        }
    };

    // Pipe the streams
    // We split both sockets into Sender and Receiver parts
    let (mut client_sender, mut _client_receiver) = client_socket.split();
    let (mut _loki_sender, mut loki_receiver) = loki_socket.split();

    // Loop: When Loki sends a message, forward it to the Client
    while let Some(msg) = loki_receiver.next().await {
        match msg {
            Ok(msg) => {
                if let Ok(text) = msg.to_text() {
                    // Loki sends JSON. You might want to parse it to just send the raw log line,
                    // or send the full JSON object to the frontend.
                    if let Err(_) = client_sender
                        .send(Message::Text(text.to_string().into()))
                        .await
                    {
                        break; // Client disconnected
                    }
                }
            }
            Err(_) => break, // Connection error
        }
    }
}
