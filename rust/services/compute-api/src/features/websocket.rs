use crate::{
    config::Config,
    error::AppError,
    features::{
        repository::DeploymentRepository,
        schemas::{LogResponse, LokiTailResponse},
    },
};
use axum::{
    extract::{
        Path, State,
        ws::{Message as WSMessage, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use compute_core::schemas::PodMetricHistory;
use factory::factories::{database::Database, redis::Redis};
use http::{HeaderName, HeaderValue, StatusCode};
use redis::{JsonAsyncCommands, aio::MultiplexedConnection};
use serde_json::json;
use tokio::time::{self, Duration};
use tracing::instrument;
use url::Url;
use users_core::jwt::Claims;
use uuid::Uuid;

use futures::{SinkExt, stream::StreamExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest as _, handshake::client::Request},
};

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
                        if socket.send(WSMessage::Text(payload.into())).await.is_err() {
                            // Client disconnected
                            break;
                        }
                    }
                    Err(err) => {
                        let payload = json!({
                            "error": err.to_string()
                        }).to_string();

                        let _ = socket.send(WSMessage::Text(payload.into())).await;
                    }
                }
            }


        }
    }
}

async fn get_deployment_metrics(
    connection: &mut MultiplexedConnection,
    deployment_id: &str,
) -> Result<Vec<PodMetricHistory>, AppError> {
    let key = format!("deployment:{}:metrics", deployment_id);
    let metrics = connection
        .json_get::<_, _, Vec<PodMetricHistory>>(&key, "$")
        .await?;
    Ok(metrics)
}

#[instrument(
    name = "stream_logs_ws_handler",
    skip_all,
    fields(
        user_id = %claims.sub,
        project_id = %project_id,
        deployment_id = %deployment_id,
    ),
    err
)]
pub async fn stream_logs_ws_handler(
    ws: WebSocketUpgrade,
    claims: Claims,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    State(cfg): State<Config>,
    State(db): State<Database>,
) -> Result<impl IntoResponse, StatusCode> {
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

    // Upgrade the connection
    Ok(ws.on_upgrade(move |socket| handle_stream_logs(request, socket)))
}

async fn handle_stream_logs(request: Request, mut client_socket: WebSocket) {
    // Connect to Loki
    let (loki_socket, _) = match connect_async(request).await {
        Ok(s) => s,
        Err(e) => {
            let _ = client_socket
                .send(WSMessage::Text(
                    format!("Error connecting to logs: {}", e).into(),
                ))
                .await;
            return;
        }
    };

    // Pipe the streams
    // We split both sockets into Sender and Receiver parts
    let (mut _loki_sender, mut loki_receiver) = loki_socket.split();
    let (mut client_sender, mut _client_receiver) = client_socket.split();

    // When Loki sends a message, forward it to the Client
    while let Some(msg) = loki_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(loki_push) = serde_json::from_str::<LokiTailResponse>(&text) {
                    let log_batch = LogResponse::from(loki_push);

                    // Yield each entry individually to the frontend
                    for entry in log_batch.entries {
                        if let Ok(json) = serde_json::to_string(&entry) {
                            if client_sender
                                .send(WSMessage::Text(json.into()))
                                .await
                                .is_err()
                            {
                                return; // Client disconnected
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            _ => {} // Ignore Ping/Pong/Binary
        }
    }
}
