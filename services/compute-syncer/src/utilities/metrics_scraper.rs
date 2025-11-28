use prometheus_http_query::Client;
use redis::aio::MultiplexedConnection;
use shared::utilities::errors::AppError;

pub async fn metrics_scraper(
    client: Client,
    connection: MultiplexedConnection,
) -> Result<(), AppError> {
    Ok(())
}
