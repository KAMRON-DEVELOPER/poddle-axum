pub struct ChannelNames;

impl ChannelNames {
    pub fn project_metrics(project_id: &str) -> String {
        format!("project:{}:metrics", project_id)
    }

    pub fn project_status(project_id: &str) -> String {
        format!("project:{}:status", project_id)
    }

    pub fn deployment_status(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:status")
    }
}
