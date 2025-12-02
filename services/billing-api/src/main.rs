pub mod features;
pub mod services;
pub mod utilities;

use std::net::SocketAddr;
use std::result::Result::Ok;

use axum::{
    extract::{ConnectInfo, DefaultBodyLimit},
    http::{self, HeaderName, HeaderValue, Method, StatusCode, header},
    response::IntoResponse,
};
use shared::{
    services::{amqp::Amqp, database::Database, kafka::Kafka, redis::Redis},
    utilities::config::Config,
};
use time::macros::format_description;
use tokio::signal;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::info;
use tracing_subscriber::{
    EnvFilter, fmt::time::LocalTime, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::utilities::app_state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    match dotenvy::dotenv() {
        Ok(path) => {
            info!("Loaded .env file from {}", path.display());
        }
        Err(dotenvy::Error::Io(ref err)) if err.kind() == std::io::ErrorKind::NotFound => {
            println!(".env file not found, continuing without it");
        }
        Err(e) => {
            println!("Couldn't load .env file: {}", e);
        }
    }

    let config = Config::init().await?;

    let filter =
        EnvFilter::new("billing_api=debug,shared=debug,tower_http=warn,hyper=warn,reqwest=warn");
    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second]"
    ));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_timer(timer),
            // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW),
        )
        .init();

    // let rustls_config = build_rustls_config(&config)?;
    let database = Database::new(&config).await?;
    let redis = Redis::new(&config).await?;
    let amqp = Amqp::new(&config).await?;
    let kafka = Kafka::new(&config, "billing-service-group")?;
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let app_state = AppState {
        rustls_config: None,
        database,
        redis,
        amqp,
        kafka,
        config: config.clone(),
        http_client,
    };

    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://127.0.0.1:3000"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("https://kronk.uz"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_credentials(true)
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            // header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderName::from_static("x-requested-with"),
        ]);

    let tracing_layer = TraceLayer::new_for_http()
        .on_request(|request: &http::Request<_>, _span: &tracing::Span| {
            let method = request.method();
            let uri = request.uri();
            let matched_path = request
                .extensions()
                .get::<axum::extract::MatchedPath>()
                .map(|p| p.as_str())
                .unwrap_or("<unknown>");

            if uri.query().is_some() {
                info!("{} {} {}", method, matched_path, uri);
            } else {
                info!("{} {}", method, matched_path);
            }
        })
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO));

    let app = axum::Router::new()
        .merge(features::routes())
        .fallback(not_found_handler)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(cors)
        .layer(tracing_layer)
        .with_state(app_state);

    info!("🚀 Server running on port {:#?}", config.server_address);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8003").await.unwrap();
    // let listener = tokio::net::TcpListener::bind(config.clone().server_addres.clone())
    //     .await
    //     .unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();

    Ok(())
}

async fn not_found_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    println!("Client with {} connected", addr);
    (StatusCode::NOT_FOUND, "nothing to see here")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
