#[tokio::main]
async fn main() {
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour

    loop {
        interval.tick().await;

        // Process ALL running deployments
        let deployments = sqlx::query!("SELECT * FROM deployments WHERE status = 'running'")
            .fetch_all(&pool)
            .await?;

        for deployment in deployments {
            let cost = calculate_hourly_cost(&deployment);

            // This triggers the DB function that deducts from balance
            sqlx::query!(
                "INSERT INTO billings (user_id, deployment_id, cost_per_hour, hours_used)
                 VALUES ($1, $2, $3, 1.0)",
                deployment.user_id,
                deployment.id,
                cost
            )
            .execute(&pool)
            .await?;
        }

        // Check for negative balances and suspend deployments
        check_and_suspend_negative_balances(&pool).await?;
    }
}
