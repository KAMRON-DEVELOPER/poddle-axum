pub mod error;
pub mod implementation;

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Query for fetching metrics for a single deployment with pods (Deployment Page)
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMetricsQuery {
    /// Relative window in minutes (default: 30)
    #[serde(default = "default_minutes")]
    pub minutes: i64,
}

/// Query for fetching metrics for multiple deployments (Project Page)
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentsMetricsQuery {
    /// Relative window in minutes (default: 30)
    #[serde(default = "default_minutes")]
    pub minutes: i64,
}

/// Query for fetching historical logs with time range
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
}

/// Query for tailing live logs (WebSocket streaming)
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TailQuery {
    pub start: i64,
}

fn default_minutes() -> i64 {
    30
}
