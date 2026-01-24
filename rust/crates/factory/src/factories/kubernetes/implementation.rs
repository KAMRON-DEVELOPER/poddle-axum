use tracing::info;

use crate::factories::kubernetes::{Kubernetes, error::KubernetesError};

impl Kubernetes {
    pub async fn new() -> Result<Self, KubernetesError> {
        let client = kube::Client::try_default().await?;
        info!("✅ Connected to kubernetes successfully");
        Ok(Self { client })
    }
}
