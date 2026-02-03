pub struct CacheKeys;

impl CacheKeys {
    /// `deployment:{}:metrics`
    pub fn deployment_metrics(id: &str) -> String {
        format!("deployment:{}:metrics", id)
    }

    /// `deployment:{id}:metrics`
    pub fn deployments_metrics(ids: &Vec<&str>) -> Vec<String> {
        ids.iter()
            .map(|id| format!("deployment:{id}:metrics"))
            .collect()
    }

    /// `deployment:{id}:pod:names`
    pub fn deployment_pod_names(id: &str) -> String {
        format!("deployment:{id}:pod:names")
    }

    /// `deployment:{id}:pod:{name}:metrics`
    pub fn deployment_pod_metrics(id: &str, name: &str) -> String {
        format!("deployment:{id}:pod:{name}:metrics")
    }

    /// `deployment:{id}:pod:{name}:metrics`
    pub fn deployment_pods_metrics(id: &str, names: &Vec<String>) -> Vec<String> {
        names
            .iter()
            .map(|name| format!("deployment:{id}:pod:{name}:metrics"))
            .collect()
    }
}
