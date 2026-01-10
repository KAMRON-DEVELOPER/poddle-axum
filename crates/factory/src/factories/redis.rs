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
    pub async fn new<T: RedisConfig>(cfg: &T) -> Result<Self, AppError> {
        let conn_info = cfg.connection_info();
        let tls_certs = cfg.tls_certificates();

        if let Some(tls_certs) = tls_certs {
            let client = Client::build_with_tls(conn_info, tls_certs)?;
            let connection = client.get_multiplexed_tokio_connection().await?;
            return Ok(Self { client, connection });
        }

        let client = Client::open(conn_info)?;
        let connection = client.get_multiplexed_tokio_connection().await?;
        Ok(Self { client, connection })
    }

    pub async fn pubsub(&self) -> Result<PubSub, AppError> {
        Ok(self.client.get_async_pubsub().await?)
    }
}
