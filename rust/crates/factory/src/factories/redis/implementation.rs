use redis::{
    Client, ClientTlsConfig, ConnectionAddr, ConnectionInfo, IntoConnectionInfo, ProtocolVersion,
    RedisConnectionInfo, TlsCertificates, aio::PubSub,
};

use crate::factories::redis::{Redis, RedisConfig, error::RedisError};

impl Redis {
    pub async fn new(cfg: &RedisConfig) -> Self {
        let conn_info = Self::connection_info(cfg);
        let tls_certs = Self::tls_certificates(cfg);

        if let Some(tls_certs) = tls_certs {
            let client = Client::build_with_tls(conn_info, tls_certs)
                .unwrap_or_else(|e| panic!("Couldn't constructs client TLS connection: {}", e));
            let connection = client
                .get_multiplexed_tokio_connection()
                .await
                .unwrap_or_else(|e| panic!("Couldn't establish connection to redis: {}", e));
            return Self { client, connection };
        }

        let client = Client::open(conn_info)
            .unwrap_or_else(|e| panic!("Couldn't connects to a redis server: {}", e));
        let connection = client
            .get_multiplexed_tokio_connection()
            .await
            .unwrap_or_else(|e| panic!("Couldn't establish connection to redis: {}", e));
        Self { client, connection }
    }

    pub async fn pubsub(&self) -> Result<PubSub, RedisError> {
        Ok(self.client.get_async_pubsub().await?)
    }

    fn connection_info(cfg: &RedisConfig) -> impl IntoConnectionInfo {
        let url = cfg.url.clone();
        let params = cfg.params.clone();

        // Prefer explicit host/port
        if let Some(params) = params.clone()
            && let Some(host) = params.host
            && let Some(port) = params.port
        {
            let conn_info = ConnectionInfo {
                addr: ConnectionAddr::Tcp(host.to_string(), port),
                redis: RedisConnectionInfo {
                    db: 0,
                    username: params.username,
                    password: params.password,
                    protocol: ProtocolVersion::RESP3,
                },
            };

            return conn_info;
        }

        // Fallback to URL
        let mut url = url.expect("Either `REDIS_URL` or `REDIS_HOST`+`REDIS_PORT` must be set");

        // Enforce RESP3
        if !url.contains("protocol=") {
            let sep = if url.contains('?') { "&" } else { "?" };
            url.push_str(sep);
            url.push_str("protocol=resp3");
        }

        // Convert URL → ConnectionInfo
        let mut conn_info = url.into_connection_info().expect("Invalid Redis URL");

        // Ensure credentials override URL if explicitly provided
        if let Some(params) = params {
            if params.username.is_some() || params.password.is_some() {
                conn_info.redis.username = params.username;
                conn_info.redis.password = params.password;
            }
        }

        conn_info
    }

    fn tls_certificates(cfg: &RedisConfig) -> Option<TlsCertificates> {
        let tls_config = cfg.tls_config.clone();

        if let Some(tls) = tls_config
            && let Some(ca) = tls.ca
            && let Some(client_cert) = tls.client_cert
            && let Some(client_key) = tls.client_key
        {
            // Structure to hold mTLS client certificate and key binaries in PEM format
            let client_tls_config = ClientTlsConfig {
                client_cert: client_cert.as_bytes().to_vec(),
                client_key: client_key.as_bytes().to_vec(),
            };

            // Structure to hold TLS certificates
            // * client_tls: binaries of clientkey and certificate within a ClientTlsConfig structure if mTLS is used
            // * root_cert: binary CA certificate in PEM format if CA is not in local truststore
            let tls_certs = TlsCertificates {
                client_tls: Some(client_tls_config),
                root_cert: Some(ca.as_bytes().to_vec()),
            };

            return Some(tls_certs);
        }

        None
    }
}
