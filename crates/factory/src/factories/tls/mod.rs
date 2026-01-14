pub mod error;
pub mod implementation;

use std::path::PathBuf;

pub trait TlsConfig {
    fn ca(&self) -> Option<String>;
    fn ca_path(&self) -> Option<PathBuf>;
    fn client_cert(&self) -> Option<String>;
    fn client_cert_path(&self) -> Option<PathBuf>;
    fn client_key(&self) -> Option<String>;
    fn client_key_path(&self) -> Option<PathBuf>;
}

pub struct Tls {
    pub ca: Option<String>,
    pub ca_path: Option<PathBuf>,
    pub client_cert: Option<String>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key: Option<String>,
    pub client_key_path: Option<PathBuf>,
}
