pub trait ProgressReporter: Send + Sync {
    fn report(
        &self,
        deployment_id: &str,
        message: &str,
    ) -> impl std::future::Future<Output = String> + Send;
}
