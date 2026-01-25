pub trait ProgressReporter: Send + Sync {
    async fn report(&self, deployment_id: &str, message: &str);
}
