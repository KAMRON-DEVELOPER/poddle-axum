pub mod error;
pub mod implementation;

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct TlsConfig {
    pub ca: Option<String>,
    pub ca_path: Option<PathBuf>,
    pub client_cert: Option<String>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key: Option<String>,
    pub client_key_path: Option<PathBuf>,
}
