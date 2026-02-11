use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::features::models::{User, UserRole, UserStatus};

#[derive(Deserialize, Debug)]
pub struct VerifyQuery {
    pub token: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RedirectResponse {
    pub redirect_to: String,
}

#[derive(Deserialize, Debug)]
pub struct OAuthCallback {
    pub(crate) code: String,
}

#[derive(Serialize, Debug)]
pub struct AuthOut {
    pub user: User,
    pub tokens: Tokens,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize, Validate, Debug)]
pub struct AuthIn {
    #[validate(length(
        min = 8,
        max = 24,
        message = "Username should be long between 8 and 24"
    ))]
    pub username: Option<String>,
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(
        min = 8,
        max = 24,
        message = "Password should be long between 8 and 24"
    ))]
    pub password: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct GoogleOAuthUser {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct GithubOAuthUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct UserIn {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub picture: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserOut {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub picture: Option<String>,
    pub role: UserRole,
    pub status: UserStatus,
    pub email_verified: bool,
    pub oauth_user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsResponse {
    pub users_total: i64,
    pub deployments_total: i64,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateFeedbackRequest {
    pub name: String,
    pub email: String,
    pub message: String,
}
