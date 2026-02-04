pub struct ChannelNames;

impl ChannelNames {
    /// `deployments:{}:metrics`
    pub fn deployments_metrics(id: &str) -> String {
        format!("project:{}:metrics", id)
    }

    /// `deployments:{}:status`
    pub fn deployments_status(id: &str) -> String {
        format!("deployments:{}:status", id)
    }

    /// `deployment:{id}:metrics`
    pub fn deployment_metrics(id: &str) -> String {
        format!("deployment:{id}:metrics")
    }

    /// `deployment:{id}:status`
    pub fn deployment_status(id: &str) -> String {
        format!("deployment:{id}:status")
    }
}
