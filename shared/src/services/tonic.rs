use crate::utilities::{config::Config, errors::AppError};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};

pub async fn build_tonic(config: &Config) -> Result<Channel, AppError> {
    let ca_certificate = Certificate::from_pem(
        &config
            .ca
            .clone()
            .ok_or_else(|| AppError::MissingTlsCaError)?,
    );
    let identity = Identity::from_pem(
        &config
            .client_cert
            .clone()
            .ok_or_else(|| AppError::MissingTlsCertError)?,
        &config
            .client_key
            .clone()
            .ok_or_else(|| AppError::MissingTlsKeyError)?,
    );

    let tls_config = ClientTlsConfig::new()
        .ca_certificate(ca_certificate)
        .identity(identity);

    let uri = format!("https://{}", config.server_address);

    let channel = Channel::from_shared(uri)?
        .tls_config(tls_config)?
        .connect()
        .await?;

    Ok(channel)
}
