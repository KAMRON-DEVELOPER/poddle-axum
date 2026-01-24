pub mod implementations;
use std::sync::Arc;

use serde::Deserialize;
use vaultrs::client::VaultClient;

#[derive(Deserialize, Clone)]
pub struct VaultConnectionSettings {
    pub address: String,
    pub name: Option<String>,
    pub skip_tls_verify: bool,
}

#[derive(Deserialize, Clone)]
pub struct VaultAuthKubernetesSettings {
    pub role: Option<String>,
    pub service_account: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct VaultAuthSettings {
    pub name: Option<String>,
    pub mount: Option<String>,
    pub kubernetes: Option<VaultAuthKubernetesSettings>,
}

#[derive(Deserialize, Clone)]
pub struct VaultSettings {
    pub kv_mount: String,
    pub vault_connection: VaultConnectionSettings,
    pub vault_auth: Option<VaultAuthSettings>,
}

#[derive(Clone)]
pub struct VaultService {
    pub client: Arc<VaultClient>,
    cfg: VaultSettings,
}
