pub struct ChannelNames;

impl ChannelNames {
    pub fn project_metrics(project_id: &str) -> String {
        format!("project:{project_id}:metrics")
    }

    pub fn project_events(project_id: &str) -> String {
        format!("project:{project_id}:events")
    }

    pub fn deployment_metrics(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:metrics")
    }

    pub fn deployment_events(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:events")
    }
}
