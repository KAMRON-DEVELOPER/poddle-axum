use compute_core::configs::PrometheusConfig;
use factory::factories::{amqp::Amqp, redis::Redis};
use kube::Client;
use serde::Deserialize;
use sqlx::PgPool;

use crate::services::vault_service::VaultService;

pub mod implementations;

#[derive(Deserialize, Clone, Debug)]
pub struct CertManagerConfig {
    pub namespace: String,
    pub cluster_issuer: String,
    pub wildcard_certificate: String,
    pub wildcard_certificate_secret: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TraefikConfig {
    pub base_domain: String,
    pub namespace: String,
    pub cluster_issuer: String,
    pub entry_points: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct KubernetesServiceConfig {
    pub kubeconfig: Option<String>,
    pub traefik: TraefikConfig,
    pub prometheus: PrometheusConfig,
    pub cert_manager: CertManagerConfig,
}

#[derive(Clone)]
pub struct KubernetesService {
    pub client: Client,
    pub pool: PgPool,
    pub redis: Redis,
    pub amqp: Amqp,
    pub vault_service: VaultService,
    pub cfg: KubernetesServiceConfig,
}
