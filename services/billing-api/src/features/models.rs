use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

// ============================================
// ENUMS
// ============================================

#[derive(Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "transaction_type", rename_all = "snake_case")]
pub enum TransactionType {
    FreeCredit,
    UsageCharge,
    Fund,
}

// ============================================
// MODELS
// ============================================

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: BigDecimal,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: Uuid,
    pub balance_id: Uuid,
    pub amount: BigDecimal,
    #[sqlx(rename = "type")]
    pub transaction_type: TransactionType,
    pub detail: Option<String>,
    pub billing_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Billing {
    pub id: Uuid,
    pub user_id: Uuid,
    pub deployment_id: Option<Uuid>,
    pub resources_snapshot: serde_json::Value,
    pub cpu_millicores: i32,
    pub memory_mb: i32,
    pub cost_per_hour: BigDecimal,
    pub hours_used: BigDecimal,
    #[sqlx(default)]
    pub total_cost: BigDecimal,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemConfig {
    pub id: bool,
    pub free_credit_enabled: bool,
    pub free_credit_amount: BigDecimal,
    pub free_credit_detail: Option<String>,
    pub updated_at: DateTime<Utc>,
}
