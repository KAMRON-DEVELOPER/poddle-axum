use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

// ============================================
// ENUMS
// ============================================

#[derive(Type, Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
#[sqlx(type_name = "transaction_type", rename_all = "snake_case")]
pub enum TransactionType {
    FreeCredit,
    UsageCharge,
    TopUp,
}

// ============================================
// MODELS
// ============================================

#[derive(FromRow, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: BigDecimal,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    // Resources
    pub cpu_millicores: i32,
    pub memory_mb: i32,
    // Pricing
    pub currency: String,
    pub monthly_price: BigDecimal,
    pub hourly_price: BigDecimal,
    // Guardrails
    pub max_addon_cpu_millicores: i32,
    pub max_addon_memory_mb: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddonPrice {
    pub id: Uuid,
    // CPU pricing
    pub cpu_monthly_unit_price: BigDecimal,
    pub cpu_hourly_unit_price: BigDecimal,
    // Memory pricing
    pub memory_monthly_unit_price: BigDecimal,
    pub memory_hourly_unit_price: BigDecimal,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: Uuid,
    pub balance_id: Uuid,
    pub billing_id: Option<Uuid>,
    pub amount: BigDecimal,
    pub detail: Option<String>,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Billing {
    // Identity
    pub id: Uuid,
    pub user_id: Uuid,
    pub deployment_id: Option<Uuid>,
    // Scaling
    pub desired_replicas: i32,
    // Preset snapshot
    pub preset_cpu_millicores: i32,
    pub preset_memory_mb: i32,
    pub preset_hourly_price: BigDecimal,
    // Addon snapshot
    pub addon_cpu_millicores: i32,
    pub addon_memory_mb: i32,
    pub addon_cpu_millicores_hourly_price: BigDecimal,
    pub addon_memory_mb_hourly_price: BigDecimal,
    // Usage (generated)
    pub cpu_millicores_used: i32,
    pub memory_mb_used: i32,
    // Time & cost
    pub hours_used: BigDecimal,
    pub total_cost: BigDecimal,
    // Metadata
    pub resources_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemConfig {
    pub id: bool,
    pub free_credit_enabled: bool,
    pub free_credit_amount: BigDecimal,
    pub free_credit_detail: Option<String>,
    pub updated_at: DateTime<Utc>,
}
