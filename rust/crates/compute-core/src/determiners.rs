use crate::models::DeploymentStatus;

pub fn determine_deployment_status(
    desired: i32,
    ready: i32,
    available: i32,
    updated: i32,
) -> DeploymentStatus {
    if desired == 0 {
        return DeploymentStatus::Suspended;
    }

    if ready == 0 && available == 0 {
        return DeploymentStatus::Starting;
    }

    if ready == desired && available == desired && updated == desired {
        return DeploymentStatus::Running;
    }

    if ready > 0 && ready < desired {
        return DeploymentStatus::Degraded;
    }

    if updated != desired {
        return DeploymentStatus::Updating;
    }

    DeploymentStatus::Unhealthy
}
