use crate::{
    error::AppError,
    features::models::{OAuthUser, Provider, User, UserRole, UserStatus},
};
use sqlx::{Executor, PgPool, Postgres, Transaction, postgres::PgQueryResult};
use uuid::Uuid;

pub struct UsersRepository;

impl UsersRepository {
    // ----------------------------------------------------------------------------
    // create_oauth_user
    // ----------------------------------------------------------------------------
    #[tracing::instrument(
        "users_repository.create_oauth_user",
        skip(picture, hash_password, provider, executor),
        err
    )]
    pub async fn create_oauth_user<'e, E>(
        id: &str,
        username: Option<&str>,
        email: Option<&str>,
        picture: Option<&str>,
        hash_password: Option<&str>,
        provider: Provider,
        executor: E,
    ) -> Result<String, AppError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        Ok(sqlx::query_scalar!(
            r#"
            INSERT INTO oauth_users (id, username, email, picture, password, provider)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            id,
            username,
            email,
            picture,
            hash_password,
            provider as Provider,
        )
        .fetch_one(executor)
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
        hash_password: Option<String>,
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
    // get_user_by_id
    // ----------------------------------------------------------------------------
    #[tracing::instrument("users_repository.get_user_by_id", skip(pool), err)]
    pub async fn get_user_by_id(id: &Uuid, pool: &PgPool) -> Result<User, AppError> {
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
            FROM users WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFoundError("User not found".to_string()))?)
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

    #[tracing::instrument("users_repository.set_user_email_verified", skip(pool), err)]
    pub async fn set_user_email_verified(
        id: &Uuid,
        pool: &PgPool,
    ) -> Result<PgQueryResult, AppError> {
        Ok(sqlx::query!(
            r#"UPDATE users
            SET email_verified = TRUE, status = 'active' WHERE id = $1
            "#,
            id
        )
        .execute(pool)
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
