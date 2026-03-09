use compute_core::models::PresetRow;
use sqlx::Executor;

use sqlx::Postgres;
use uuid::Uuid;

pub struct DeploymentPresetRepository;

impl DeploymentPresetRepository {
    #[tracing::instrument(name = "deployment_preset_repository.get_by_id", skip(executor), err)]
    pub async fn get_by_id<'e, E>(preset_id: &Uuid, executor: E) -> Result<PresetRow, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            PresetRow,
            r#"
            SELECT *
            FROM presets
            WHERE id = $1
            "#,
            preset_id
        )
        .fetch_one(executor)
        .await
    }
}
