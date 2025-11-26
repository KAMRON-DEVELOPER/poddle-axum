use prometheus_http_query::Client;
use redis::aio::MultiplexedConnection;

pub async fn metrics_scraper(client: Client, connection: MultiplexedConnection) {}
