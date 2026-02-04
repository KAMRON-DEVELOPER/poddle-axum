pub struct ChannelNames;

impl ChannelNames {
    /// `project:{}:metrics`
    pub fn project_metrics(id: &str) -> String {
        format!("project:{}:metrics", id)
    }

    /// `project:{}:status`
    pub fn project_status(id: &str) -> String {
        format!("project:{}:status", id)
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
