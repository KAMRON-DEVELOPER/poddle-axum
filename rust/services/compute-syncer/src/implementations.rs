use factory::factories::{
    amqp::AmqpConfig,
    database::DatabaseConfig,
    kubernetes::KubernetesConfig,
    redis::{RedisConfig, RedisParams},
    tls::TlsConfig,
};
use sqlx::postgres::PgSslMode;

use crate::config::Config;

// -------------------------------------------------------------------------------
// ---------------------------- Error implementations ----------------------------
// -------------------------------------------------------------------------------

// -------------------------------------------------------------------------------
// --------------------------- Factory implementations ---------------------------
// -------------------------------------------------------------------------------

impl KubernetesConfig for Config {
    fn k8s_in_cluster(&self) -> bool {
        self.k8s_in_cluster.clone()
    }

    fn k8s_config_path(&self) -> Option<String> {
        self.k8s_config_path.clone()
    }
}

impl DatabaseConfig for Config {
    type Tls = TlsConfig;

    fn url(&self) -> String {
        self.database_url.clone()
    }
    fn max_connections(&self) -> u32 {
        self.postgres_pool_size.unwrap_or_default()
    }
    fn pg_ssl_mode(&self) -> PgSslMode {
        self.pg_ssl_mode
    }
    fn tls_config(&self) -> Self::Tls {
        TlsConfig {
            ca: self.ca.clone(),
            ca_path: self.ca_path.clone(),
            client_cert: self.client_cert.clone(),
            client_cert_path: self.client_cert_path.clone(),
            client_key: self.client_key.clone(),
            client_key_path: self.client_key_path.clone(),
        }
    }
}

impl RedisConfig for Config {
    type Tls = TlsConfig;

    fn url(&self) -> Option<String> {
        self.redis_url.clone()
    }

    fn params(&self) -> RedisParams {
        RedisParams {
            host: self.redis_host.clone(),
            port: self.redis_port.clone(),
            username: self.redis_username.clone(),
            password: self.redis_password.clone(),
        }
    }

    fn tls_config(&self) -> Self::Tls {
        TlsConfig {
            ca: self.ca.clone(),
            ca_path: self.ca_path.clone(),
            client_cert: self.client_cert.clone(),
            client_cert_path: self.client_cert_path.clone(),
            client_key: self.client_key.clone(),
            client_key_path: self.client_key_path.clone(),
        }
    }
}

impl AmqpConfig for Config {
    type Tls = TlsConfig;

    fn uri(&self) -> String {
        self.amqp_addr.clone()
    }

    fn tls_config(&self) -> Self::Tls {
        TlsConfig {
            ca: self.ca.clone(),
            ca_path: self.ca_path.clone(),
            client_cert: self.client_cert.clone(),
            client_cert_path: self.client_cert_path.clone(),
            client_key: self.client_key.clone(),
            client_key_path: self.client_key_path.clone(),
        }
    }
}
