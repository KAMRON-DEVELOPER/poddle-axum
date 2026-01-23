use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;
use vaultrs::client::{Client, VaultClient, VaultClientSettingsBuilder};

use vaultrs::auth::kubernetes;
use vaultrs::kv2;

use crate::config::Config;
use crate::error::AppError;

#[derive(Clone)]
pub struct VaultService {
    pub client: Arc<VaultClient>,
    pub kv_mount: String,
    pub address: String,
    pub skip_tls_verify: bool,
    pub vault_connection: String,
    pub vault_auth: String,
}

impl VaultService {
    pub async fn init(config: &Config) -> Result<Self, AppError> {
        info!("🔐 Initializing Vault client");

        let mut client = VaultClient::new(
            VaultClientSettingsBuilder::default()
                .address(&config.vault_address)
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
            kv_mount: config.vault_kv_mount.clone(),
            address: config.vault_address.clone(),
            skip_tls_verify: config.vault_skip_tls_verify,
            vault_connection: config.vault_connection.clone(),
            vault_auth: config.vault_auth.clone(),
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
