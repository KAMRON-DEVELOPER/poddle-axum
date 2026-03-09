use crate::models::{DeploymentEventLevel, DeploymentEventType, DeploymentStatus};

pub fn resolve_event_level(
    status: Option<DeploymentStatus>,
    level: Option<DeploymentEventLevel>,
) -> DeploymentEventLevel {
    if let Some(level) = level {
        return level;
    }

    if let Some(status) = status {
        return map_status_to_event_level(status);
    }

    DeploymentEventLevel::Info
}

pub fn resolve_event_type(
    status: Option<DeploymentStatus>,
    event_type: Option<DeploymentEventType>,
) -> DeploymentEventType {
    if let Some(event_type) = event_type {
        return event_type;
    }

    if let Some(status) = status {
        return map_status_to_event_type(status);
    }

    DeploymentEventType::SystemMessage
}

pub fn map_status_to_event_type(status: DeploymentStatus) -> DeploymentEventType {
    match status {
        DeploymentStatus::BuildFailed => DeploymentEventType::BuildFailed,
        DeploymentStatus::Unhealthy => DeploymentEventType::UnhealthyDetected,
        DeploymentStatus::ImagePullError => DeploymentEventType::ImagePullFailed,

        DeploymentStatus::Building
        | DeploymentStatus::Queued
        | DeploymentStatus::Provisioning
        | DeploymentStatus::Starting
        | DeploymentStatus::Running
        | DeploymentStatus::Degraded
        | DeploymentStatus::Updating
        | DeploymentStatus::Suspended
        | DeploymentStatus::Failed
        | DeploymentStatus::Deleted => DeploymentEventType::StatusChanged,
    }
}

pub fn map_status_to_event_level(status: DeploymentStatus) -> DeploymentEventLevel {
    match status {
        DeploymentStatus::Failed
        | DeploymentStatus::BuildFailed
        | DeploymentStatus::ImagePullError => DeploymentEventLevel::Error,

        DeploymentStatus::Unhealthy | DeploymentStatus::Degraded | DeploymentStatus::Suspended => {
            DeploymentEventLevel::Warning
        }

        _ => DeploymentEventLevel::Info,
    }
}
