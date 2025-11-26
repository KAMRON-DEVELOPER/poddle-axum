use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use crate::features::models::{Balance, Transaction};

pub struct BillingRepository;

impl BillingRepository {
    pub async fn get_user_balance(pool: &PgPool, user_id: Uuid) -> Result<Balance, sqlx::Error> {
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

    pub async fn get_transactions(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<Transaction>, sqlx::Error> {
        sqlx::query_as::<_, Transaction>(
            r#"
            SELECT t.*
            FROM transactions t
            INNER JOIN balances b ON t.balance_id = b.id
            WHERE b.user_id = $1
            ORDER BY t.created_at DESC
            LIMIT 100
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }
}
