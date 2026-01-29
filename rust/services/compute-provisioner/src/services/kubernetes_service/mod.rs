use compute_core::configs::PrometheusConfig;
use kube::Client;
use serde::Deserialize;

use crate::services::vault_service::VaultService;

pub mod implementations;

#[derive(Deserialize, Clone, Debug)]
pub struct CertManagerConfig {
    pub namespace: String,
    pub cluster_issuer: String,
    pub wildcard_certificate: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TraefikConfig {
    pub base_domain: String,
    pub namespace: String,
    // pub cluster_issuer: String,
    pub entry_points: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct KubernetesServiceConfig {
    pub kubeconfig: Option<String>,
    pub otel_exporter_otlp_endpoint: Option<String>,
    pub traefik: TraefikConfig,
    pub prometheus: PrometheusConfig,
    pub cert_manager: CertManagerConfig,
}

#[derive(Clone)]
pub struct KubernetesService {
    pub client: Client,
    pub cfg: KubernetesServiceConfig,
    pub vault_service: VaultService,
}
