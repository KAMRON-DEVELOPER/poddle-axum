use http_contracts::pagination::schema::Pagination;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use crate::features::models::{AddonPrice, Balance, Preset, Transaction, TransactionType};

pub struct BillingRepository;

impl BillingRepository {
    #[tracing::instrument(name = "billing_repository.get_balance", skip_all, fields(user_id = %user_id), err)]
    pub async fn get_balance(user_id: Uuid, pool: &PgPool) -> Result<Balance, sqlx::Error> {
        sqlx::query_as::<Postgres, Balance>(
            r#"
                SELECT id, user_id, amount, currency, created_at, updated_at
                FROM balances
                WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
    }

    #[tracing::instrument(name = "billing_repository.get_presets", skip_all, err)]
    pub async fn get_presets(pool: &PgPool) -> Result<Vec<Preset>, sqlx::Error> {
        sqlx::query_as::<Postgres, Preset>(r#"SELECT * FROM presets"#)
            .fetch_all(pool)
            .await
    }

    #[tracing::instrument(name = "billing_repository.get_addon_price", skip_all, err)]
    pub async fn get_addon_price(pool: &PgPool) -> Result<AddonPrice, sqlx::Error> {
        sqlx::query_as::<Postgres, AddonPrice>(r#"SELECT * FROM addon_prices"#)
            .fetch_one(pool)
            .await
    }

    #[tracing::instrument(name = "billing_repository.get_transactions", skip_all, fields(user_id = %user_id), err)]
    pub async fn get_transactions(
        user_id: Uuid,
        pagination: Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<Transaction>, i64), sqlx::Error> {
        // In standard SQL, if you use COUNT(*), the database "collapses" all your rows into a single number.
        // You lose your individual deployment data.
        // OVER() turns the count into a Window Function.
        // It tells Postgres: "Calculate the total count of all rows that match the WHERE clause, but don't collapse them."
        // The exclamation mark (!) is specific to the sqlx::query! macro in Rust. It is called a `Force Non-Null Override`.
        let rows = sqlx::query!(
            r#"
            SELECT
                t.id,
                t.balance_id,
                t.billing_id,
                t.amount,
                t.detail,
                t.type AS "type: TransactionType",
                t.created_at,
                COUNT(*) OVER() as "total!"
            FROM transactions t
            INNER JOIN balances b ON t.balance_id = b.id
            WHERE b.user_id = $1
            ORDER BY t.created_at DESC
            LIMIT $2
            OFFSET $3
            "#,
            user_id,
            pagination.limit,
            pagination.offset
        )
        .fetch_all(pool)
        .await?;

        // Without that !, your code would have to look like this
        // let total = rows.get(0).map(|r| r.total.unwrap_or(0)).unwrap_or(0);
        // With the !, it's much cleaner
        let total = rows.get(0).map(|r| r.total).unwrap_or(0);

        let transactions = rows
            .into_iter()
            .map(|r| Transaction {
                id: r.id,
                balance_id: r.balance_id,
                billing_id: r.billing_id,
                amount: r.amount,
                detail: r.detail,
                transaction_type: r.r#type,
                created_at: r.created_at,
            })
            .collect();

        Ok((transactions, total))
    }
}
