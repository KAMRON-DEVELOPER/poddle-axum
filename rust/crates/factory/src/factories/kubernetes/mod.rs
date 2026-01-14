pub mod error;
pub mod implementation;

use kube::Client;

pub trait KubernetesConfig {
    fn k8s_in_cluster(&self) -> bool;
    fn k8s_config_path(&self) -> Option<String>;
}

#[derive(Clone)]
pub struct Kubernetes {
    pub client: Client,
}
