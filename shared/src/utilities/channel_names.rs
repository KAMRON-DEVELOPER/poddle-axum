pub struct ChannelNames;

impl ChannelNames {
    pub fn deployment_status(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:status")
    }

    pub fn deployment_metrics(deployment_id: &str) -> String {
        format!("deployment:{deployment_id}:metrics")
    }
}
