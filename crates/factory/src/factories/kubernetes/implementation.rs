use kube::{
    Client, Config,
    config::{KubeConfigOptions, Kubeconfig},
};
use tracing::info;

use crate::factories::kubernetes::{Kubernetes, error::KubernetesError};

pub trait KubernetesConfig {
    fn k8s_in_cluster(&self) -> bool;
    fn k8s_config_path(&self) -> Option<String>;
}

impl Kubernetes {
    pub async fn new<T: KubernetesConfig>(config: &T) -> Result<Self, KubernetesError> {
        // let client = kube::Client::try_default().await?;
        let client = if config.k8s_in_cluster() {
            let kube_config = Config::incluster()?;
            info!("✅ Connected from incluster environment!");
            Client::try_from(kube_config)?
        } else {
            let kube_config = if let Some(path) = &config.k8s_config_path() {
                let kubeconfig = Kubeconfig::read_from(path)?;
                let options = KubeConfigOptions::default();
                Config::from_custom_kubeconfig(kubeconfig, &options).await?
            } else {
                Config::infer().await?
            };

            info!("✅ Connected from local environment!");
            Client::try_from(kube_config)?
        };

        Ok(Self { client })
    }
}
