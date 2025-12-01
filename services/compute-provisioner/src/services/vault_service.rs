use std::collections::HashMap;
use std::sync::Arc;

use shared::utilities::config::Config;
use shared::utilities::errors::AppError;
use uuid::Uuid;
use vaultrs::client::{Client, VaultClient, VaultClientSettingsBuilder};

use vaultrs::auth::kubernetes;
use vaultrs::kv2;

#[derive(Clone)]
pub struct VaultService {
    pub client: Arc<VaultClient>,
    pub kv_mount: String,
}

impl VaultService {
    pub async fn init(config: &Config) -> Result<Self, AppError> {
        let jwt = tokio::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/token")
            .await?;

        let mut client = VaultClient::new(
            VaultClientSettingsBuilder::default()
                .address(&config.vault_address)
                .build()?,
        )?;

        let auth_info = kubernetes::login(
            &client,
            &config.vault_auth_mount,
            &config.vault_auth_role,
            &jwt,
        )
        .await?;

        client.set_token(&auth_info.client_token);

        Ok(Self {
            client: Arc::new(client),
            kv_mount: config.vault_auth_mount.clone(),
        })
    }

    /// Store deployment secrets in Vault
    pub async fn store_secrets(
        &self,
        deployment_id: Uuid,
        secrets: HashMap<String, String>,
    ) -> Result<String, AppError> {
        let path = format!("deployments/{}", deployment_id);

        kv2::set(&*self.client, &self.kv_mount, &path, &secrets)
            .await
            .map_err(|e| {
                AppError::InternalError(format!("Failed to store secrets in Vault: {}", e))
            })?;

        Ok(path)
    }

    /// Read deployment secrets from Vault
    pub async fn read_secrets(
        &self,
        deployment_id: Uuid,
    ) -> Result<HashMap<String, String>, AppError> {
        let path = format!("deployments/{}", deployment_id);

        let secret = kv2::read(&*self.client, &self.kv_mount, &path)
            .await
            .map_err(|e| {
                AppError::InternalError(format!("Failed to read secrets from Vault: {}", e))
            })?;

        Ok(secret)
    }

    /// Update deployment secrets
    pub async fn update_secrets(
        &self,
        deployment_id: Uuid,
        secrets: HashMap<String, String>,
    ) -> Result<(), AppError> {
        let path = format!("deployments/{}", deployment_id);

        kv2::set(&*self.client, &self.kv_mount, &path, &secrets)
            .await
            .map_err(|e| {
                AppError::InternalError(format!("Failed to update secrets in Vault: {}", e))
            })?;

        Ok(())
    }

    /// Delete deployment secrets
    pub async fn delete_secrets(&self, deployment_id: Uuid) -> Result<(), AppError> {
        let path = format!("deployments/{}", deployment_id);

        kv2::delete_latest(&*self.client, &self.kv_mount, &path)
            .await
            .map_err(|e| {
                AppError::InternalError(format!("Failed to delete secrets from Vault: {}", e))
            })?;

        Ok(())
    }

    /// Get secret keys
    pub async fn get_secret_keys(&self, deployment_id: Uuid) -> Result<Vec<String>, AppError> {
        let secrets = self.read_secrets(deployment_id).await?;
        Ok(secrets.keys().cloned().collect())
    }
}
