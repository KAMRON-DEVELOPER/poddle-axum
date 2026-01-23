use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

// ============================================
// ENUMS
// ============================================

#[derive(Type, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    #[default]
    Regular,
}

#[derive(Type, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[sqlx(type_name = "user_status", rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
    #[default]
    PendingVerification,
}

#[derive(Type, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[sqlx(type_name = "provider", rename_all = "snake_case")]
pub enum Provider {
    Google,
    Github,
    Email,
}

// ============================================
// MODELS
// ============================================

#[derive(FromRow, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUser {
    pub id: String,
    pub provider: Provider,
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub picture: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password: Option<String>,
    pub picture: Option<String>,
    pub email_verified: bool,
    pub role: UserRole,
    pub status: UserStatus,
    pub oauth_user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub device_name: Option<String>,
    pub refresh_token: Option<String>,
    pub is_active: bool,
    pub last_activity_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
