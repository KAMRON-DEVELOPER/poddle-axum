use uuid::Uuid;

pub struct CacheKeys;

impl CacheKeys {
    pub fn deployment_metrics(deployment_ids: &Vec<Uuid>) -> Vec<String> {
        deployment_ids
            .iter()
            .map(|id| format!("deployment:{}:metrics", id))
            .collect()
    }
}
