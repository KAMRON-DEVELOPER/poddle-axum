use http_contracts::pagination::schema::Pagination;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use crate::features::models::{AddonPrice, Balance, Preset, Transaction, TransactionType};

pub struct BillingRepository;

impl BillingRepository {
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

    pub async fn get_presets(user_id: Uuid, pool: &PgPool) -> Result<Vec<Preset>, sqlx::Error> {
        sqlx::query_as::<Postgres, Preset>(
            r#"
                SELECT *
                FROM presets
                WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn get_addon_price(user_id: Uuid, pool: &PgPool) -> Result<AddonPrice, sqlx::Error> {
        sqlx::query_as::<Postgres, AddonPrice>(
            r#"
                SELECT *
                FROM addon_prices
                WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
    }

    pub async fn get_transactions(
        user_id: Uuid,
        pagination: Pagination,
        pool: &PgPool,
    ) -> Result<(Vec<Transaction>, i64), sqlx::Error> {
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
        .await;

        // In standard SQL, if you use COUNT(*), the database "collapses" all your rows into a single number.
        // You lose your individual deployment data.
        // OVER() turns the count into a Window Function.
        // It tells Postgres: "Calculate the total count of all rows that match the WHERE clause, but don't collapse them."
        // The exclamation mark (!) is specific to the sqlx::query! macro in Rust. It is called a `Force Non-Null Override`.
        let rows = sqlx::query!(
            r#"
            SELECT
                d.id,
                d.user_id,
                d.project_id,
                d.name,
                d.image,
                d.port,
                d.desired_replicas,
                d.ready_replicas,
                d.available_replicas,
                d.preset_id,
                d.addon_cpu_millicores,
                d.addon_memory_mb,
                d.vault_secret_path,
                d.secret_keys,
                d.environment_variables AS "environment_variables: Json<Option<HashMap<String, String>>>",
                d.labels AS "labels: Json<Option<HashMap<String, String>>>",
                d.status AS "status: DeploymentStatus",
                d.domain,
                d.subdomain,
                d.created_at,
                d.updated_at,
                COUNT(*) OVER() as "total!"
            FROM deployments d
            INNER JOIN projects p ON d.project_id = p.id
            WHERE p.owner_id = $1 AND d.project_id = $2
            ORDER BY d.created_at DESC
            LIMIT $3
            OFFSET $4
            "#,
            user_id,
            project_id,
            pagination.limit,
            pagination.offset
        )
        .fetch_all(pool)
        .await?;

        // Without that !, your code would have to look like this
        // let total = rows.get(0).map(|r| r.total.unwrap_or(0)).unwrap_or(0);
        // With the !, it's much cleaner
        let total = rows.get(0).map(|r| r.total).unwrap_or(0);

        let deployments = rows
            .into_iter()
            .map(|r| Deployment {
                id: r.id,
                user_id: r.user_id,
                project_id: r.project_id,
                name: r.name,
                image: r.image,
                port: r.port,
                desired_replicas: r.desired_replicas,
                ready_replicas: r.ready_replicas,
                available_replicas: r.available_replicas,
                preset_id: r.preset_id,
                addon_cpu_millicores: r.addon_cpu_millicores,
                addon_memory_mb: r.addon_memory_mb,
                vault_secret_path: r.vault_secret_path,
                secret_keys: r.secret_keys,
                environment_variables: r.environment_variables,
                labels: r.labels,
                status: r.status,
                domain: r.domain,
                subdomain: r.subdomain,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();

        Ok((total, deployments))
    }
}
