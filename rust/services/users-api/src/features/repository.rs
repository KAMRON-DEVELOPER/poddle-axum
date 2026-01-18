use crate::{
    error::AppError,
    features::models::{OAuthUser, Provider, User, UserRole, UserStatus},
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct UsersRepository;

impl UsersRepository {
    // ----------------------------------------------------------------------------
    // create_oauth_user
    // ----------------------------------------------------------------------------
    #[tracing::instrument(
        "users_repository.create_oauth_user",
        skip(hash_password, provider, tx),
        err
    )]
    pub async fn create_oauth_user(
        username: &str,
        email: &str,
        hash_password: &str,
        provider: Provider,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<String, AppError> {
        Ok(sqlx::query_scalar!(
            r#"
            INSERT INTO oauth_users (username, email, password, provider)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
            username,
            email,
            hash_password,
            provider as Provider,
        )
        .fetch_one(&mut **tx)
        .await?)
    }

    // ----------------------------------------------------------------------------
    // find_oauth_user_by_email
    // ----------------------------------------------------------------------------
    #[tracing::instrument("users_repository.find_oauth_user_by_email", skip(pool), err)]
    pub async fn find_oauth_user_by_email(
        email: &str,
        pool: &PgPool,
    ) -> Result<Option<OAuthUser>, AppError> {
        Ok(sqlx::query_as!(
            OAuthUser,
            r#"
            SELECT
                id,
                provider AS "provider: Provider",
                username,
                email,
                password,
                picture,
                created_at,
                updated_at
            FROM oauth_users WHERE email = $1
            "#,
            email,
        )
        .fetch_optional(pool)
        .await?)
    }

    // ----------------------------------------------------------------------------
    // create_user
    // ----------------------------------------------------------------------------
    #[tracing::instrument(
        "users_repository.create_user",
        skip(hash_password, oauth_user_id, tx),
        err
    )]
    pub async fn create_user(
        username: String,
        email: String,
        hash_password: String,
        oauth_user_id: String,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<User, AppError> {
        Ok(sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, email, password, oauth_user_id)
            VALUES ($1,$2,$3,$4)
            RETURNING
                id,
                username,
                email,
                password,
                picture,
                role AS "role: UserRole",
                status AS "status: UserStatus",
                email_verified,
                oauth_user_id,
                created_at,
                updated_at
            "#,
            username,
            email,
            hash_password,
            oauth_user_id
        )
        .fetch_one(&mut **tx)
        .await?)
    }

    // ----------------------------------------------------------------------------
    // find_user_by_email
    // ----------------------------------------------------------------------------
    #[tracing::instrument("users_repository.find_user_by_email", skip(pool), err)]
    pub async fn find_user_by_email(email: &str, pool: &PgPool) -> Result<Option<User>, AppError> {
        Ok(sqlx::query_as!(
            User,
            r#"
            SELECT
                id, 
                username,
                email,
                password,
                picture,
                role AS "role: UserRole",
                status AS "status: UserStatus",
                email_verified,
                oauth_user_id,
                created_at,
                updated_at
            FROM users WHERE email = $1
            "#,
            email,
        )
        .fetch_optional(pool)
        .await?)
    }

    // ----------------------------------------------------------------------------
    // create_session
    // ----------------------------------------------------------------------------
    #[tracing::instrument(
        "users_repository.create_session",
        skip(pool, user_id, user_agent, ip_address, refresh_token),
        err
    )]
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
}
