use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;
use vaultrs::client::{Client, VaultClient, VaultClientSettingsBuilder};

use vaultrs::auth::kubernetes;
use vaultrs::kv2;

use crate::settings::Config;
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


K8S_IN_CLUSTER=false
K8S_KUBECONFIG='/home/kamronbek/.kube/config'
# Run: kubectl create token compute-provisioner -n poddle-system --duration=87600h
K8S_SA_TOKEN='eyJhbGciOiJSUzI1NiIsImtpZCI6Inl1RnZyQWFicEtTZjlKUFlBWV85SExyUm5hb0JyZFUwZFJFSWlzbGpENUkifQ.eyJhdWQiOlsiaHR0cHM6Ly9rdWJlcm5ldGVzLmRlZmF1bHQuc3ZjLmNsdXN0ZXIubG9jYWwiLCJrM3MiXSwiZXhwIjoyMDc5OTQ0MzcwLCJpYXQiOjE3NjQ1ODQzNzAsImlzcyI6Imh0dHBzOi8va3ViZXJuZXRlcy5kZWZhdWx0LnN2Yy5jbHVzdGVyLmxvY2FsIiwianRpIjoiZGM2MTNlYjgtNjY1MS00NDNmLTkzMGUtZjc0OWRlYTY0NTJhIiwia3ViZXJuZXRlcy5pbyI6eyJuYW1lc3BhY2UiOiJwb2RkbGUtc3lzdGVtIiwic2VydmljZWFjY291bnQiOnsibmFtZSI6ImNvbXB1dGUtcHJvdmlzaW9uZXIiLCJ1aWQiOiJjYTdkYzNhMC02MTE3LTRjYmUtOWYxNS04MTM1NmIxM2Q5NTYifX0sIm5iZiI6MTc2NDU4NDM3MCwic3ViIjoic3lzdGVtOnNlcnZpY2VhY2NvdW50OnBvZGRsZS1zeXN0ZW06Y29tcHV0ZS1wcm92aXNpb25lciJ9.D4-UqJRPoCdDWj9Tb-AfcNqC-jEzV5il8gv2BKWE1Bp93bpnsxKrpxXhbJtneWQDcxvZE2dYuI3vVMhiOqRoONaJMNMwjE6J_lOlcVyrxTueqVqjUS56JscgRF4K44DNNJb7elfyrutlUI-52TRVASGXoQc6tUgTjDHT9WxOEGktvTdvzVmyWej9CyAYLPkGornUgOYHXlPxWK3V3tOG2e1Lyofgwp1_o5Jmv6gGWLplSPBIIDKH3Om6jIqIrrFi6X1sxIK-Pl8nsrrBNb4vCUUsUqz7534TTUZGYJRZ8EEvm1PbLUuq50eH3Y_gbo80hrNw3a1ETXs-bjDU5bdf9A'

LABEL_SELECTOR='managed-by=poddle'
SCRAPE_INTERVAL_SECONDS='15'
METRIC_SNAPSHOTS_TO_KEEP='240'

VAULT_ADDR='https://vault.poddle.uz:8200'
VAULT_AUTH_MOUNT='kubernetes'
VAULT_AUTH_ROLE='compute-provisioner'
VAULT_KV_MOUNT='kvv2'
VAULT_SKIP_TLS_VERIFY=true
VAULT_CONNECTION='vault-connection'
VAULT_AUTH='vault-auth'
REFRESH_AFTER='300s'



KUBERNETES_CONFIG='{
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

VAULT_CONFIG='{
   "vault_connection": {
       "address": "https://vault.poddle.uz:8200",
       "name": "vault-connection",
       "skip_tls_verify": true
   },
   "vault_auth": {
       "name": "vault-auth",
       "mount": "kubernetes",
       "kubernetes": {
       "role": "tenant-role",
       "service_account": "default"
       },
   },
   "vault_static_secret": {
        "refresh_after": "300s"
   }
}'

DOMAIN='poddle.uz'
CLUSTER_ISSUER_NAME='vault-k8s-ci'
TRAEFIK_NAMESPACE='traefik'
INGRESSROUTE_ENTRY_POINTS='web,websecure'
WILDCARD_CERTIFICATE_NAME='wildcard-poddle-uz-tls-certificate'
WILDCARD_CERTIFICATE_COMMON_NAME=''
WILDCARD_CERTIFICATE_DURATION='720h' # 30 days
WILDCARD_CERTIFICATE_RENEW_BEFORE='168h' # Renew 7 days before expiry
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
