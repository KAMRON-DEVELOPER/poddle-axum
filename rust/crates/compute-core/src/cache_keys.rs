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

    /// `deployment:{id}:pods`
    pub fn deployment_pods(id: &str) -> String {
        format!("deployment:{id}:pods")
    }

    /// `deployment:{id}:pod:{uid}:meta`
    pub fn deployment_pod_meta(id: &str, uid: &str) -> String {
        format!("deployment:{id}:pod:{uid}:meta")
    }

    /// `deployment:{id}:pod:{uid}:meta`
    pub fn deployment_pods_meta(id: &str, uids: &Vec<String>) -> String {
        uids.iter()
            .map(|uid| format!("deployment:{id}:pod:{uid}:meta"))
            .collect()
    }

    /// `deployment:{id}:pod:{uid}:metrics`
    pub fn deployment_pod_metrics(id: &str, uid: &str) -> String {
        format!("deployment:{id}:pod:{uid}:metrics")
    }

    // `deployment:{id}:pod:{uid}:metrics`
    // pub fn deployment_pods_metrics(id: &str, uids: &Vec<String>) -> Vec<String> {
    //     uids.iter()
    //         .map(|uid| format!("deployment:{id}:pod:{uid}:metrics"))
    //         .collect()
    // }
}
