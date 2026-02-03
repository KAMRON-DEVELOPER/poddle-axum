pub struct ChannelNames;

impl ChannelNames {
    pub fn project_metrics(id: &str) -> String {
        format!("project:{}:metrics", id)
    }

    pub fn project_status(id: &str) -> String {
        format!("project:{}:status", id)
    }

    pub fn deployment_metrics(id: &str) -> String {
        format!("deployment:{id}:metrics")
    }

    pub fn deployment_status(id: &str) -> String {
        format!("deployment:{id}:status")
    }
}
