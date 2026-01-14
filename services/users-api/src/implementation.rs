impl DatabaseConfig for Config {
    type Tls = Tls;

    fn database_url(&self) -> String {
        self.database_url.clone()
    }
    fn max_connections(&self) -> u32 {
        self.postgres_pool_size.unwrap_or_default()
    }
    fn pg_ssl_mode(&self) -> PgSslMode {
        self.pg_ssl_mode
    }
    fn tls_config(&self) -> Self::Tls {
        Tls {
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
    fn connection_info(&self) -> impl IntoConnectionInfo {}
}

impl AmqpConfig for Config {
    type Tls = Tls;

    fn uri(&self) -> String {
        self.amqp_addr.clone()
    }

    fn tls_config(&self) -> Self::Tls {
        Tls {
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
