pub struct CacheKeys;

impl CacheKeys {
    pub fn deployment_metrics(id: &str) -> String {
        format!("deployment:{}:metrics", id)
    }

    pub fn deployments_metrics(ids: &Vec<&str>) -> Vec<String> {
        ids.iter()
            .map(|id| format!("deployment:{}:metrics", id))
            .collect()
    }

    pub fn pod_metrics(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:pods")
    }

    pub fn pods_metrics(pod_ids: &Vec<&str>) -> Vec<String> {
        pod_ids
            .iter()
            .map(|id| format!("pod:{}:metrics", id))
            .collect()
    }
}
