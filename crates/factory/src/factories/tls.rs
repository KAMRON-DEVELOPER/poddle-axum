use std::{
    fs::File,
    io::{BufReader, Cursor},
};

use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer},
};
use rustls_pemfile::{Item, read_one};

use shared::utilities::{config::Config, errors::AppError};

pub trait TlsConfig {
    fn ca(&self) -> String;
    fn client_cert(&self) -> String;
    fn client_key(&self) -> String;
}

/// Build TLS client config from Config.
pub fn build_rustls_config(config: &Config) -> Result<ClientConfig, AppError> {
    let ca: Option<String> = config.ca.clone();
    let ca_path: Option<std::path::PathBuf> = config.ca_path.clone();
    let client_cert: Option<String> = config.client_cert.clone();
    let client_cert_path: Option<std::path::PathBuf> = config.client_cert_path.clone();
    let client_key: Option<String> = config.client_key.clone();
    let client_key_path: Option<std::path::PathBuf> = config.client_key_path.clone();

    if ca.is_none() && ca_path.is_none() {
        return Err(AppError::MissingTlsCaError);
    }
    if client_cert.is_none() && client_cert_path.is_none() {
        return Err(AppError::MissingTlsCertError);
    }
    if client_key.is_none() && client_key_path.is_none() {
        return Err(AppError::MissingTlsKeyError);
    }

    // --- 1. Load Root CA ---
    let mut root_store: RootCertStore = RootCertStore::empty();

    {
        // Prefer in-memory string if present
        if let Some(ca_str) = ca {
            let mut reader = Cursor::new(ca_str.as_bytes());

            match read_one(&mut reader) {
                Ok(Some(Item::X509Certificate(cert))) => {
                    root_store.add(cert)?;
                }
                Ok(Some(_)) => {
                    return Err(AppError::IncompatibleCaCertTypeError(
                        "Unsupported PEM type for CA cert".to_string(),
                    ));
                }
                Ok(None) => {
                    return Err(AppError::InvalidCaCertError(
                        "No certificate found in PEM".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(AppError::InvalidCaCertError(format!(
                        "Failed to read PEM: {e}"
                    )));
                }
            }

            // let cert = CertificateDer::from_pem_slice(ca_str.as_bytes())?;
            // let cert = CertificateDer::from_pem_slice(ca_str.as_bytes())
            //     .map_err(|e| AppError::InvalidCaCertError(e.to_string()))?;
            // root_store.add(cert);
        }
        // Fallback to filesystem
        else if let Some(path) = ca_path {
            let file = File::open(path)?;
            let mut reader = BufReader::new(file);

            match read_one(&mut reader) {
                Ok(Some(Item::X509Certificate(cert))) => {
                    root_store.add(cert)?;
                }
                Ok(Some(_)) => {
                    return Err(AppError::IncompatibleCaCertTypeError(
                        "Unsupported PEM type for CA cert".to_string(),
                    ));
                }
                Ok(None) => {
                    return Err(AppError::InvalidCaCertError(
                        "No certificate found in PEM".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(AppError::InvalidCaCertError(format!(
                        "Failed to read PEM: {e}"
                    )));
                }
            }

            // for cert in rustls_pemfile::certs(&mut reader) {
            //     let cert = cert?;
            //     root_store.add(cert)?;
            // }
        }
    }

    // --- 2. Load Client Certificate Chain ---
    let client_certs: Vec<CertificateDer> = {
        // Prefer in-memory string if present
        if let Some(cert_str) = client_cert {
            let mut reader = Cursor::new(cert_str.as_bytes());

            let mut certs = Vec::new();
            loop {
                match read_one(&mut reader) {
                    Ok(Some(Item::X509Certificate(cert))) => {
                        certs.push(cert);
                    }
                    Ok(Some(_)) => {
                        return Err(AppError::IncompatibleCaCertTypeError(
                            "Unsupported PEM type in client certificate".to_string(),
                        ));
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(AppError::InvalidCaCertError(format!(
                            "Failed to read client cert PEM: {e}"
                        )));
                    }
                }
            }

            if certs.is_empty() {
                return Err(AppError::InvalidCaCertError(
                    "No client certificates found in PEM".to_string(),
                ));
            }
            certs
        }
        // Fallback to filesystem
        else if let Some(cert_path) = client_cert_path {
            let file = File::open(cert_path)?;
            let mut reader = BufReader::new(file);

            let mut certs = Vec::new();
            loop {
                match read_one(&mut reader) {
                    Ok(Some(Item::X509Certificate(cert))) => {
                        certs.push(cert);
                    }
                    Ok(Some(_)) => {
                        return Err(AppError::IncompatibleCaCertTypeError(
                            "Unsupported PEM type in client certificate file".to_string(),
                        ));
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(AppError::InvalidCaCertError(format!(
                            "Failed to read client cert PEM file: {e}"
                        )));
                    }
                }
            }

            if certs.is_empty() {
                return Err(AppError::InvalidCaCertError(
                    "No client certificates found in file".to_string(),
                ));
            }
            certs
        } else {
            return Err(AppError::MissingTlsCertError); // Missing client cert
        }
    };

    // --- 3. Load Client Private Key ---
    let client_key: PrivateKeyDer = {
        if let Some(key_str) = client_key {
            let mut reader = Cursor::new(key_str.as_bytes());
            match read_one(&mut reader) {
                Ok(Some(Item::Pkcs8Key(key))) => PrivateKeyDer::Pkcs8(key),
                Ok(Some(Item::Pkcs1Key(key))) => PrivateKeyDer::Pkcs1(key),
                Ok(Some(Item::Sec1Key(key))) => PrivateKeyDer::Sec1(key),
                Ok(Some(_)) => {
                    return Err(AppError::IncompatibleClientCertTypeError(
                        "Unsupported PEM type in client key".to_string(),
                    ));
                }
                Ok(None) => {
                    return Err(AppError::InvalidClientCertError(
                        "No private key found in PEM".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(AppError::InvalidClientCertError(format!(
                        "Failed to read client key PEM: {e}"
                    )));
                }
            }
        } else if let Some(key_path) = client_key_path {
            let file = File::open(key_path)?;
            let mut reader = BufReader::new(file);
            match read_one(&mut reader) {
                Ok(Some(Item::Pkcs8Key(key))) => PrivateKeyDer::Pkcs8(key),
                Ok(Some(Item::Pkcs1Key(key))) => PrivateKeyDer::Pkcs1(key),
                Ok(Some(Item::Sec1Key(key))) => PrivateKeyDer::Sec1(key),
                Ok(Some(_)) => {
                    return Err(AppError::IncompatibleClientCertTypeError(
                        "Unsupported PEM type in client key file".to_string(),
                    ));
                }
                Ok(None) => {
                    return Err(AppError::InvalidClientCertError(
                        "No private key found in PEM file".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(AppError::InvalidClientCertError(format!(
                        "Failed to read client key PEM file: {e}"
                    )));
                }
            }
        } else {
            return Err(AppError::MissingTlsKeyError); // Missing client key
        }
    };

    // --- 4. Build ClientConfig ---
    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, client_key)?;

    Ok(client_config)
}
