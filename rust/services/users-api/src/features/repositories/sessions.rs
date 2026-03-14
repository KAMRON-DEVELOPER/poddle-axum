use sqlx::{PgPool, postgres::PgQueryResult};
use uuid::Uuid;

pub struct SessionsRepository;

impl SessionsRepository {
    // ----------------------------------------------------------------------------
    // create
    // ----------------------------------------------------------------------------
    #[tracing::instrument("sessions_repository.create", skip_all, err)]
    pub async fn create(
        user_id: &Uuid,
        user_agent: &str,
        ip_address: &str,
        refresh_token: &str,
        pool: &PgPool,
    ) -> Result<PgQueryResult, sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO sessions (user_id, user_agent, ip_address, refresh_token)
            VALUES ($1, $2, $3, $4)
            "#,
            user_id,
            user_agent,
            ip_address,
            refresh_token
        )
        .execute(pool)
        .await
    }
}
