use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use compute_core::github_app::{GithubApp, Installation};
use factory::factories::database::Database;
use reqwest::Client;
use serde::Deserialize;
use users_core::jwt::Claims;

use crate::{config::Config, error::AppError};

#[derive(Deserialize)]
pub struct CallbackParams {
    installation_id: i64,
    setup_action: Option<String>, // usually "install" or "update"
}

pub async fn github_callback(
    claims: Claims,
    State(github_app): State<GithubApp>,
    State(cfg): State<Config>,
    State(http): State<Client>,
    State(db): State<Database>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    let jwt = github_app.generate_jwt().expect("Failed to sign JWT");

    let res = http
        .get(format!(
            "https://api.github.com/app/installations/{}",
            params.installation_id
        ))
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "poddle-compute")
        .send()
        .await?;

    let status = res.status();
    if status == 200 {
        let ins = res.json::<Installation>().await?;
        let account_id = ins.account.id;
        let account_login = ins.account.login;
        let account_type = ins.account.account_type;

        sqlx::query!(
            r#"
            INSERT INTO installations 
                (user_id, installation_id, account_login, account_id, account_type)
            VALUES ($1, $2, 'github', $3, $4, $5)
            ON CONFLICT (user_id, installation_id) DO NOTHING
            "#,
            user_id,
            params.installation_id,
            account_login,
            account_id,
            account_type
        )
        .execute(&db.pool)
        .await
        .expect("Failed to save integration");
    } else if status == 404 {
        return Err(AppError::BadRequest("installation id is invalid".into()));
    } else {
        return Err(AppError::InternalServerError("Something went wrong".into()));
    }

    let redirect = Redirect::to(&format!(
        "{}/dashboard?github_connected=true",
        cfg.frontend_endpoint
    ));
    Ok(redirect.into_response())
}

pub async fn get_github_repos(
    claims: Claims,
    State(github_app): State<GithubApp>,
    State(http): State<Client>,
    State(db): State<Database>,
) -> Result<Json<Value>, String> {
    // 1. Get Installation ID from DB
    let integration = sqlx::query!(
        "SELECT installation_id FROM git_integrations WHERE user_id = $1 LIMIT 1",
        user.id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let installation_id = match integration {
        Some(i) => i.installation_id,
        None => return Err("GITHUB_NOT_CONNECTED".to_string()), // Frontend should show "Connect" button
    };

    // 2. Generate App JWT
    let app_jwt = state
        .github_app
        .generate_jwt(state.config.github_app.clone())
        .map_err(|_| "Failed to generate JWT".to_string())?;

    // 3. Exchange JWT for Installation Access Token
    let client = reqwest::Client::new();
    let token_resp = client
        .post(format!(
            "https://api.github.com/app/installations/{}/access_tokens",
            installation_id
        ))
        .header("Authorization", format!("Bearer {}", app_jwt))
        .header("User-Agent", "Poddle-Compute")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !token_resp.status().is_success() {
        return Err("Failed to get access token from GitHub".to_string());
    }

    let token_data: serde_json::Value = token_resp.json().await.map_err(|e| e.to_string())?;
    let access_token = token_data["token"].as_str().unwrap();

    // 4. List Repositories
    // We use the 'installation/repositories' endpoint which lists ONLY repos the user selected
    let repos_resp = client
        .get("https://api.github.com/installation/repositories")
        .header("Authorization", format!("token {}", access_token))
        .header("User-Agent", "Poddle-Compute")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let repos_json: serde_json::Value = repos_resp.json().await.map_err(|e| e.to_string())?;

    // Return the list (The array is usually under the key "repositories")
    Ok(Json(repos_json))
}
