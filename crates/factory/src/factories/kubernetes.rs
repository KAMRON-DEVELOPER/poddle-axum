use kube::{
    Client, Config as KubeConfig,
    config::{KubeConfigOptions, Kubeconfig},
};
use shared::utilities::{config::Config, errors::AppError};
use tracing::info;

#[derive(Clone)]
pub struct Kubernetes {
    pub client: Client,
}

impl Kubernetes {
    pub async fn new(config: &Config) -> Result<Self, AppError> {
        // let client = kube::Client::try_default().await?;
        let client = if config.k8s_in_cluster {
            let kube_config = KubeConfig::incluster()?;
            info!("Connected from incluster environment!");
            Client::try_from(kube_config)?
        } else {
            let kube_config = if let Some(path) = &config.k8s_config_path {
                let kubeconfig = Kubeconfig::read_from(path)?;
                let options = KubeConfigOptions::default();
                KubeConfig::from_custom_kubeconfig(kubeconfig, &options).await?
            } else {
                KubeConfig::infer().await?
            };

            info!("✅ Connected from local environment!");
            Client::try_from(kube_config)?
        };

        Ok(Self { client })
    }
}
