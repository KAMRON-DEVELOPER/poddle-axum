use sqlx::{Executor, PgPool, Postgres, postgres::PgQueryResult};
use tracing::instrument;

use crate::features::{models::Feedback, schemas::CreateFeedbackRequest};

pub struct FeedbacksRepository;

impl FeedbacksRepository {
    // ----------------------------------------------------------------------------
    // create
    // ----------------------------------------------------------------------------
    #[instrument("feedbacks_repository.create", skip_all, err)]
    pub async fn create<'e, E>(
        req: &CreateFeedbackRequest,
        executor: E,
    ) -> Result<PgQueryResult, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query!(
            r#"
            INSERT INTO feedbacks (name, email, message)
            VALUES ($1, $2, $3)
            "#,
            req.name,
            req.email,
            req.message,
        )
        .execute(executor)
        .await
    }

    // ----------------------------------------------------------------------------
    // get_many
    // ----------------------------------------------------------------------------
    #[tracing::instrument("feedbacks_repository.get_many", skip_all, err)]
    pub async fn get_many(
        offset: i64,
        limit: i64,
        pool: &PgPool,
    ) -> Result<(Vec<Feedback>, i64), sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                *,
                COUNT(*) OVER() as "total!"
            FROM feedbacks
            ORDER BY created_at DESC
            OFFSET $1
            LIMIT $2
            "#,
            offset,
            limit
        )
        .fetch_all(pool)
        .await?;

        let total = rows.get(0).map(|r| r.total).unwrap_or(0);

        let feedbacks = rows
            .into_iter()
            .map(|r| Feedback {
                id: r.id,
                name: r.name,
                email: r.email,
                message: r.message,
                created_at: r.created_at,
            })
            .collect();

        Ok((feedbacks, total))
    }
}
