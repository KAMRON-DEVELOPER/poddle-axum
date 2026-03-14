use crate::features::models::{OAuthUser, Provider};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

pub struct OAuthUsersRepository;

impl OAuthUsersRepository {
    // ----------------------------------------------------------------------------
    // create
    // ----------------------------------------------------------------------------
    #[tracing::instrument("oauth_users_repository.create", skip_all, err)]
    pub async fn create<'e, E>(
        OAuthUser {
            id,
            provider,
            user_id,
            username,
            email,
            picture,
            ..
        }: OAuthUser,
        executor: E,
    ) -> Result<OAuthUser, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            OAuthUser,
            r#"
            INSERT INTO oauth_users (id, provider, user_id, username, email, picture)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id,
                provider AS "provider: Provider",
                user_id,
                username,
                email,
                picture,
                created_at,
                updated_at
            "#,
            id,
            provider as Provider,
            user_id,
            username,
            email,
            picture
        )
        .fetch_one(executor)
        .await
    }

    // ----------------------------------------------------------------------------
    // find
    // ----------------------------------------------------------------------------
    #[tracing::instrument("oauth_users_repository.find", skip_all, err)]
    pub async fn find<'e, E>(
        id: &str,
        provider: &Provider,
        executor: E,
    ) -> Result<Option<OAuthUser>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
            OAuthUser,
            r#"
            SELECT
                id,
                provider AS "provider: Provider",
                user_id,
                username,
                email,
                picture,
                created_at,
                updated_at
            FROM oauth_users
            WHERE id = $1 AND provider = $2
            "#,
            id,
            provider as &Provider
        )
        .fetch_optional(executor)
        .await
    }

    #[tracing::instrument("oauth_users_repository.find_providers_by_user_id", skip_all, err)]
    pub async fn find_providers_by_user_id(
        user_id: &Uuid,
        pool: &PgPool,
    ) -> Result<Vec<Provider>, sqlx::Error> {
        sqlx::query_scalar!(
            r#"
            SELECT provider AS "provider: Provider"
            FROM oauth_users
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
    }
}
