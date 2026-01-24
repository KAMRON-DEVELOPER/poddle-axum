pub mod error;
pub mod implementation;

use kube::Client;

#[derive(Clone)]
pub struct Kubernetes {
    pub client: Client,
}
