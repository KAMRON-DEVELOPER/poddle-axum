use factory::factories::redis::Redis;
use kube::Client;
use serde::Deserialize;
use sqlx::PgPool;

use crate::services::vault_service::VaultService;

pub mod implementations;

#[derive(Deserialize, Clone)]
pub struct CertManagerSettings {
    pub namespace: String,
    pub cluster_issuer: String,
}

#[derive(Deserialize, Clone)]
pub struct TraefikSettings {
    pub base_domain: String,
    pub namespace: String,
    pub cluster_issuer: String,
    pub entry_points: Option<Vec<String>>,
    pub wildcard_certificate: String,
    pub wildcard_certificate_secret: String,
}

#[derive(Deserialize, Clone)]
pub struct KubernetesSettings {
    pub kubeconfig: String,
    pub sa_token: Option<String>,
    pub label_selector: String,
    pub scrape_interval_seconds: String,
    pub metric_snapshots_to_keep: String,
    pub cert_manager: CertManagerSettings,
    pub traefik: TraefikSettings,
}

#[derive(Clone)]
pub struct KubernetesService {
    pub client: Client,
    pub pool: PgPool,
    pub redis: Redis,
    pub vault_service: VaultService,
    pub domain: String,
    pub traefik_namespace: String,
    pub cluster_issuer_name: String,
    pub ingress_class_name: Option<String>,
    pub ingressroute_entry_points: Option<Vec<String>>,
    pub wildcard_certificate_name: String,
    pub wildcard_certificate_secret_name: String,
}
