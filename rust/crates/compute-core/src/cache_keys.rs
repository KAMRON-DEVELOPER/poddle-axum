pub struct CacheKeys;

impl CacheKeys {
    /// `deployment:{id}:metrics`
    pub fn deployment_metrics(id: &str) -> String {
        format!("deployment:{id}:metrics")
    }
    /// `deployment:{id}:pods`
    pub fn deployment_pods(id: &str) -> String {
        format!("deployment:{id}:pods")
    }

    /// `deployment:{id}:pod:{uid}:meta`
    pub fn deployment_pod_meta(id: &str, uid: &str) -> String {
        format!("deployment:{id}:pod:{uid}:meta")
    }

    /// `deployment:{id}:pod:{uid}:metrics`
    pub fn deployment_pod_metrics(id: &str, uid: &str) -> String {
        format!("deployment:{id}:pod:{uid}:metrics")
    }
}
