use crate::{
    error::AppError,
    features::{
        models::{User, UserRole, UserStatus},
        schemas::UserMutationPayload,
    },
};
use sqlx::{Executor, PgPool, Postgres, Transaction, postgres::PgQueryResult};
use uuid::Uuid;

pub struct UsersRepository;

impl UsersRepository {
    // ----------------------------------------------------------------------------
    // create
    // ----------------------------------------------------------------------------
    #[tracing::instrument("users_repository.create", skip_all, err)]
    pub async fn create(
        UserMutationPayload {
            username,
            email,
            hash_password,
            picture,
        }: UserMutationPayload,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<User, AppError> {
        Ok(sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, email, password, picture)
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
                created_at,
                updated_at
            "#,
            username,
            email,
            hash_password,
            picture
        )
        .fetch_one(&mut **tx)
        .await?)
    }

    // ----------------------------------------------------------------------------
    // get
    // ----------------------------------------------------------------------------

    #[tracing::instrument("users_repository.get", skip_all, err)]
    pub async fn get<'e, E>(id: &Uuid, executor: E) -> Result<User, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
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
                created_at,
                updated_at
            FROM users WHERE id = $1
            "#,
            id
        )
        .fetch_one(executor)
        .await
    }

    // ----------------------------------------------------------------------------
    // find_by_email
    // ----------------------------------------------------------------------------
    #[tracing::instrument("users_repository.find_by_email", skip_all, err)]
    pub async fn find_by_email<'e, E>(email: &str, executor: E) -> Result<Option<User>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_as!(
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
                created_at,
                updated_at
            FROM users WHERE email = $1
            "#,
            email,
        )
        .fetch_optional(executor)
        .await
    }

    #[tracing::instrument("users_repository.set_email_verified", skip_all, err)]
    pub async fn set_email_verified(
        id: &Uuid,
        pool: &PgPool,
    ) -> Result<PgQueryResult, sqlx::Error> {
        sqlx::query!(
            r#"UPDATE users
            SET email_verified = TRUE, status = 'active' WHERE id = $1
            "#,
            id
        )
        .execute(pool)
        .await
    }

    #[tracing::instrument("users_repository.update_password", skip_all, err)]
    pub async fn update_password(
        user_id: &Uuid,
        hash_password: &str,
        pool: &PgPool,
    ) -> Result<PgQueryResult, AppError> {
        Ok(sqlx::query!(
            r#"UPDATE users SET password = $1 WHERE id = $2"#,
            hash_password,
            user_id
        )
        .execute(pool)
        .await?)
    }
}
