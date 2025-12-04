pub struct CacheKeys;

// deployment:{uuid}:metrics:aggregate
// deployment:{uuid}:pod:{pod_name}:metrics

impl CacheKeys {
    pub fn deployment_status(deployment_id: &str) -> String {
        format!("deployment:{}:status", deployment_id)
    }

    pub fn deployment_summary(deployment_id: &str) -> String {
        format!("deployment:{}:summary", deployment_id)
    }

    pub fn deployment_detailed(deployment_id: &str) -> String {
        format!("deployment:{}:detailed", deployment_id)
    }

    /// metric_type: "cpu" or "memory"
    pub fn deployment_history(deployment_id: &str, metric_type: &str) -> String {
        format!(
            "metrics:deployment:{}:history:{}",
            deployment_id, metric_type
        )
    }

    /// metric_type: "cpu" or "memory"
    pub fn pod_history(namespace: &str, pod_name: &str, metric_type: &str) -> String {
        format!(
            "metrics:pod:{}:{}:history:{}",
            namespace, pod_name, metric_type
        )
    }
}
