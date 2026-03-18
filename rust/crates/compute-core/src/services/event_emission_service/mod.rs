pub mod error;

use chrono::{DateTime, Utc};
use redis::{AsyncTypedCommands, aio::MultiplexedConnection};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    channel_names::ChannelNames,
    event::ComputeEvent,
    helpers::map_status_to_event_level,
    models::{DeploymentEventLevel, DeploymentEventType, DeploymentStatus},
    repository::{DeploymentEventRepository, DeploymentRepository},
    services::event_emission_service::error::EventEmissionServiceError,
};

#[derive(Serialize, Deserialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentEventUpdate {
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
    // pub deployment_name: &'a str,
    pub status: Option<DeploymentStatus>,
    pub event_type: Option<DeploymentEventType>,
    pub level: DeploymentEventLevel,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct DeploymentEventEmitterInput<'a> {
    pub project_id: &'a Uuid,
    pub deployment_id: &'a Uuid,
    // pub deployment_name: &'a str,
    pub status: Option<DeploymentStatus>,
    pub event_type: Option<DeploymentEventType>,
    pub level: Option<DeploymentEventLevel>,
    pub message: Option<&'a str>,
    pub persist_event: bool,
    pub publish_project: bool,
    pub publish_deployment: bool,
}

pub struct DeploymentEventEmitter;

impl DeploymentEventEmitter {
    #[tracing::instrument(
        name = "deployment_event_emitter.emit",
        skip_all,
        fields(
            project_id = %input.project_id,
            deployment_id = %input.deployment_id,
            status = ?input.status,
            event_type = ?input.event_type
        ),
        err
    )]
    pub async fn emit(
        input: DeploymentEventEmitterInput<'_>,
        pool: &PgPool,
        con: &mut MultiplexedConnection,
    ) -> Result<(), EventEmissionServiceError> {
        let level = input
            .level
            .or_else(|| input.status.map(map_status_to_event_level))
            .unwrap_or(DeploymentEventLevel::Info);

        let mut persisted_id = None;
        let mut created_at = Utc::now();

        if let Some(status) = input.status {
            let res =
                DeploymentRepository::update_status(input.deployment_id, status, pool).await?;
            if res.rows_affected() == 0 {
                tracing::warn!("deployment status update affected zero rows");
            }
        }

        if input.persist_event {
            if let Some(event_type) = input.event_type {
                let row = DeploymentEventRepository::create(
                    input.project_id,
                    input.deployment_id,
                    event_type,
                    level,
                    input.message,
                    pool,
                )
                .await?;

                persisted_id = Some(row.id);
                created_at = row.created_at;
            }
        }

        let message = ComputeEvent::DeploymentEvent {
            event: DeploymentEventUpdate {
                id: persisted_id,
                project_id: *input.project_id,
                deployment_id: *input.deployment_id,
                // deployment_name: &*input.deployment_name,
                status: input.status,
                event_type: input.event_type,
                level,
                message: input.message.map(str::to_string),
                created_at,
            },
        };

        if input.publish_project {
            let channel = ChannelNames::project_events(&input.project_id.to_string());
            con.publish(channel, &message).await?;
        }

        if input.publish_deployment {
            let channel = ChannelNames::deployment_events(&input.deployment_id.to_string());
            con.publish(channel, message).await?;
        }

        Ok(())
    }
}
