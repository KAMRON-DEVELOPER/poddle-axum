use serde::Serialize;

use crate::{
    schemas::{DeploymentMetricUpdate, Pod, PodMetricUpdate, PodPhase},
    services::event_emission_service::DeploymentEventEnvelope,
};

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ComputeEvent<'a> {
    DeploymentMetricsUpdate {
        updates: Vec<DeploymentMetricUpdate>,
    },
    DeploymentEvent {
        event: DeploymentEventEnvelope,
    },

    PodMetricsUpdate {
        updates: Vec<PodMetricUpdate>,
    },
    PodPhaseUpdate {
        uid: &'a str,
        status: PodPhase,
    },
    PodApply {
        pod: Pod,
    },
    PodDelete {
        uid: String,
    },
}
