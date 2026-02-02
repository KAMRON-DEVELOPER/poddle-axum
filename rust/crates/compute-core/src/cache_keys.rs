pub struct CacheKeys;

impl CacheKeys {
    pub fn deployment_metrics(deployment_id: &str) -> String {
        format!("deployment:{}:metrics", deployment_id)
    }

    pub fn deployments_metrics(ids: &Vec<&str>) -> Vec<String> {
        ids.iter()
            .map(|id| format!("deployment:{}:metrics", id))
            .collect()
    }

    pub fn pod_metrics(pod_id: &str) -> String {
        format!("pod:{}:metrics", pod_id)
    }

    pub fn pods_metrics(pod_ids: &Vec<&str>) -> Vec<String> {
        pod_ids
            .iter()
            .map(|id| format!("pod:{}:metrics", id))
            .collect()
    }
}
