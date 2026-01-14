use thiserror::Error;

#[derive(Error, Debug)]
pub enum KubernetesError {
    #[error("Kube error")]
    KubeError(#[from] kube::Error),
    #[error("InClusterError, {0}")]
    InClusterError(#[from] kube_client::config::InClusterError),
    #[error("KubeconfigError, {0}")]
    KubeconfigError(#[from] kube_client::config::KubeconfigError),
    #[error("InferConfigError, {0}")]
    InferConfigError(#[from] kube_client::config::InferConfigError),
}
