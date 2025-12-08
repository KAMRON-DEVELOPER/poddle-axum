use uuid::Uuid;

pub struct CacheKeys;

impl CacheKeys {
    pub fn deployment_metrics(deployment_id: &Uuid) -> String {
        format!("deployment:{}:metrics", deployment_id)
    }

    pub fn deployments_metrics(deployment_ids: &Vec<Uuid>) -> Vec<String> {
        deployment_ids
            .iter()
            .map(|id| format!("deployment:{}:metrics", id))
            .collect()
    }

    pub fn pod_metrics(pod_id: &Uuid) -> String {
        format!("pod:{}:metrics", pod_id)
    }

    pub fn pods_metrics(pod_ids: &Vec<Uuid>) -> Vec<String> {
        pod_ids
            .iter()
            .map(|id| format!("pod:{}:metrics", id))
            .collect()
    }
}
