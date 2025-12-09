pub struct ChannelNames;

impl ChannelNames {
    pub fn deployment_status(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:status")
    }

    pub fn deployment_metrics(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:metrics")
    }

    pub fn deployment_metrics_in_project(project_id: &str, deployment_id: &str) -> String {
        format!(
            "project:{}:deployment:{}:metrics",
            project_id, deployment_id
        )
    }

    pub fn project_deployments_pattern(project_id: &str) -> String {
        format!("project:{}:deployment:*:metrics", project_id)
    }
}
