use factory::factories::{
    amqp::AmqpConfig,
    database::DatabaseConfig,
    mailtrap::error::MailtrapError,
    redis::{RedisConfig, RedisParams},
    tls::TlsConfig,
    zepto::error::ZeptoError,
};
use sqlx::postgres::PgSslMode;
use users_core::jwt::JwtConfig;

use crate::config::Config;

use users_core::error::JwtError;

use crate::error::AppError;

// -------------------------------------------------------------------------------
// ---------------------------- Error implementations ----------------------------
// -------------------------------------------------------------------------------

impl From<JwtError> for AppError {
    fn from(e: JwtError) -> Self {
        match e {
            JwtError::Creation => AppError::TokenCreationError,
            JwtError::Invalid => AppError::InvalidTokenError,
            JwtError::Expired => AppError::ExpiredTokenError,
            JwtError::WrongType => AppError::WrongTokenTypeError,
        }
    }
}

impl From<ZeptoError> for AppError {
    fn from(err: ZeptoError) -> Self {
        match err {
            ZeptoError::Api { error } => AppError::ExternalServiceError {
                service: "ZeptoMail".to_string(),
                code: error.code,
                message: error.message,
            },
            ZeptoError::Request(_) => AppError::ServiceUnavailable("ZeptoMail".to_string()),
            ZeptoError::Deserialize(e) => AppError::InternalServerError(e.to_string()),
        }
    }
}

impl From<MailtrapError> for AppError {
    fn from(err: MailtrapError) -> Self {
        match err {
            MailtrapError::Api { error } => AppError::ExternalServiceError {
                service: "Mailtrap".to_string(),
                code: "".to_string(),
                message: error.errors.join(","),
            },
            MailtrapError::Request(_) => AppError::ServiceUnavailable("Mailtrap".to_string()),
            MailtrapError::Deserialization(e) => AppError::InternalServerError(e.to_string()),
        }
    }
}

// -------------------------------------------------------------------------------
// --------------------------- Factory implementations ---------------------------
// -------------------------------------------------------------------------------

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

impl JwtConfig for Config {
    fn jwt_secret(&self) -> &str {
        &self.jwt_secret_key
    }

    fn access_token_expire_in_minute(&self) -> i64 {
        self.access_token_expire_in_minute
    }

    fn refresh_token_expire_in_days(&self) -> i64 {
        self.refresh_token_expire_in_days
    }

    fn email_verification_token_expire_in_hours(&self) -> i64 {
        self.email_verification_token_expire_in_hours
    }
}
