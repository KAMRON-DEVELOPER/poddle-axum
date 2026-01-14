use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_session(
    pool: &PgPool,
    user_id: &Uuid,
    user_agent: &str,
    ip_address: &str,
    refresh_token: &str,
) -> Result<(), AppError> {
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
    .await?;

    Ok(())
}
