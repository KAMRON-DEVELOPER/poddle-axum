use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;
use vaultrs::client::{Client, VaultClient, VaultClientSettingsBuilder};

use vaultrs::auth::kubernetes;
use vaultrs::kv2;

use crate::error::AppError;
use crate::services::vault_service::{
    VaultAuthConfig, VaultAuthKubernetesConfig, VaultServiceConfig, VaultService,
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

impl VaultService {
    pub async fn init(cfg: &VaultServiceConfig) -> Result<Self, AppError> {
        info!("🔐 Initializing Vault client");

        let mut client = VaultClient::new(
            VaultClientSettingsBuilder::default()
                .address(&cfg.vault_connection.address)
                .build()?,
        )?;

        let auth_info =
            kubernetes::login(&client, &cfg.auth_mount, &cfg.auth_role, &cfg.auth_jwt).await?;

        client.set_token(&auth_info.client_token);

        Ok(Self {
            client: Arc::new(client),
            cfg: cfg.clone(),
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

        kv2::set(&*self.client, &self.cfg.kv_mount, &path, &secrets)
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

        let secret = kv2::read(&*self.client, &self.cfg.kv_mount, &path)
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

        kv2::set(&*self.client, &self.cfg.kv_mount, &path, &secrets)
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

        kv2::delete_latest(&*self.client, &self.cfg.kv_mount, &path)
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
