use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;
use vaultrs::client::{Client, VaultClient, VaultClientSettingsBuilder};

use vaultrs::auth::kubernetes;
use vaultrs::kv2;

use crate::config::Config;
use crate::error::AppError;
use crate::services::vault_service::{
    VaultAuthConfig, VaultAuthKubernetesConfig, VaultConfig, VaultService,
};

impl Default for VaultAuthKubernetesConfig {
    fn default() -> Self {
        Self {
            role: Some("tenant-role".to_string()),
            service_account: Default::default(),
        }
    }
}

impl Default for VaultAuthConfig {
    fn default() -> Self {
        Self {
            name: Some("vault-auth".to_string()),
            mount: Some("kubernetes".to_string()),
            kubernetes: Default::default(),
        }
    }
}

/*
VAULT_ADDR='https://vault.poddle.uz:8200'
VAULT_AUTH_MOUNT='kubernetes'
VAULT_AUTH_ROLE='compute-provisioner'
VAULT_KV_MOUNT='kvv2'
VAULT_SKIP_TLS_VERIFY=true
VAULT_CONNECTION='vault-connection'
VAULT_AUTH='vault-auth'
REFRESH_AFTER='300s'

VAULT_CONFIG='{
   "vault_connection": {
       "address": "https://vault.poddle.uz:8200",
       "name": "vault-connection",
       "skip_tls_verify": true
   }
   "vault_auth": {
       "name": "vault-auth",
       "mount": "kubernetes",
       "kubernetes": {
       "role": "tenant-role",
       "service_account": "default"
       }
   }
}'
*/

impl VaultService {
    pub async fn init(cfg: VaultConfig) -> Result<Self, AppError> {
        info!("🔐 Initializing Vault client");

        let mut client = VaultClient::new(
            VaultClientSettingsBuilder::default()
                .address(&cfg.vault_connection.address)
                .build()?,
        )?;

        let auth_info = kubernetes::login(
            &client,
            &config.vault_auth_mount,
            &config.vault_auth_role,
            &config.k8s_sa_token,
        )
        .await?;

        client.set_token(&auth_info.client_token);

        Ok(Self {
            client: Arc::new(client),
            cfg,
        })
    }

    /// Store deployment secrets in Vault
    pub async fn store_secrets(
        &self,
        namespace: &str,
        deployment_id: &str,
        secrets: HashMap<String, String>,
    ) -> Result<String, AppError> {
        let path = format!("{}/{}", namespace, deployment_id);

        kv2::set(&*self.client, &self.kv_mount, &path, &secrets)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to store secrets in Vault: {}", e))
            })?;

        Ok(path)
    }

    /// Read deployment secrets from Vault
    pub async fn read_secrets(
        &self,
        namespace: &str,
        deployment_id: &str,
    ) -> Result<HashMap<String, String>, AppError> {
        let path = format!("{}/{}", namespace, deployment_id);

        let secret = kv2::read(&*self.client, &self.kv_mount, &path)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to read secrets from Vault: {}", e))
            })?;

        Ok(secret)
    }

    /// Update deployment secrets
    pub async fn update_secrets(
        &self,
        namespace: &str,
        deployment_id: &str,
        secrets: HashMap<String, String>,
    ) -> Result<(), AppError> {
        let path = format!("{}/{}", namespace, deployment_id);

        kv2::set(&*self.client, &self.kv_mount, &path, &secrets)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to update secrets in Vault: {}", e))
            })?;

        Ok(())
    }

    /// Delete deployment secrets
    pub async fn delete_secrets(
        &self,
        namespace: &str,
        deployment_id: &str,
    ) -> Result<(), AppError> {
        let path = format!("{}/{}", namespace, deployment_id);

        kv2::delete_latest(&*self.client, &self.kv_mount, &path)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to delete secrets from Vault: {}", e))
            })?;

        Ok(())
    }

    /// Get secret keys
    pub async fn get_secret_keys(
        &self,
        namespace: &str,
        deployment_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let secrets = self.read_secrets(namespace, deployment_id).await?;
        Ok(secrets.keys().cloned().collect())
    }
}
