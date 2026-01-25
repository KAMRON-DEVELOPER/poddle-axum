pub mod implementations;
use std::sync::Arc;

use serde::Deserialize;
use vaultrs::client::VaultClient;

#[derive(Deserialize, Clone, Debug)]
pub struct VaultConnectionConfig {
    pub address: String,
    pub name: Option<String>,
    pub skip_tls_verify: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VaultAuthKubernetesConfig {
    pub role: Option<String>,
    pub service_account: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VaultAuthConfig {
    pub name: Option<String>,
    pub mount: Option<String>,
    pub k8s: VaultAuthKubernetesConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VaultStaticSecretConfig {
    pub refresh_after: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VaultServiceConfig {
    pub address: String,
    pub auth_mount: String,
    pub auth_role: String,
    pub auth_jwt: String,
    pub kv_mount: String,
    pub vault_connection: Option<VaultConnectionConfig>,
    pub vault_auth: VaultAuthConfig,
    pub vault_static_secret: VaultStaticSecretConfig,
}

#[derive(Clone)]
pub struct VaultService {
    pub client: Arc<VaultClient>,
    pub cfg: VaultServiceConfig,
}
