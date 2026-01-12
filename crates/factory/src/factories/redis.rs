use redis::{
    Client, IntoConnectionInfo, TlsCertificates,
    aio::{MultiplexedConnection, PubSub},
};
use shared::utilities::errors::AppError;

pub trait RedisConfig {
    fn connection_info(&self) -> impl IntoConnectionInfo {
        "redis://localhost:6379/0"
            .into_connection_info()
            .expect("Invalid Redis default URL")
    }
    fn tls_certificates(&self) -> Option<TlsCertificates> {
        None
    }
}

#[derive(Clone)]
pub struct Redis {
    pub client: Client,
    pub connection: MultiplexedConnection,
}

impl Redis {
    pub async fn new<T: RedisConfig>(cfg: &T) -> Self {
        let conn_info = cfg.connection_info();
        let tls_certs = cfg.tls_certificates();

        if let Some(tls_certs) = tls_certs {
            let client = Client::build_with_tls(conn_info, tls_certs)
                .unwrap_or_else(|e| panic!("Couldn't constructs client TLS connection: {}", e));
            let connection = client.get_multiplexed_tokio_connection().await.unwrap_or_else(|e| {
                panic!("Couldn't establish connection to redis: {}", e)
            });
            return Self { client, connection };
        }

        let client = Client::open(conn_info)
            .unwrap_or_else(|e| panic!("Couldn't connects to a redis server: {}", e));
        let connection = client
            .get_multiplexed_tokio_connection()
            .await
            .unwrap_or_else(|e| {
                panic!("Couldn't establish connection to redis: {}", e)
            });
        Self { client, connection }
    }

    pub async fn pubsub(&self) -> Result<PubSub, AppError> {
        Ok(self.client.get_async_pubsub().await?)
    }
}
