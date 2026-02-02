use chrono::{DateTime, Duration, Utc};
use compute_core::schemas::ProjectPageQuery;
use http_contracts::pagination::schema::Pagination;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct ProjectPageWithPaginationQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    #[serde(flatten)]
    pub project_page_query: ProjectPageQuery,
}

#[derive(Deserialize, Debug)]
pub struct LogQuery {
    #[serde(default = "default_start")]
    pub start: DateTime<Utc>,
    #[serde(default = "default_end")]
    pub end: DateTime<Utc>,
}

fn default_start() -> DateTime<Utc> {
    Utc::now() - Duration::minutes(15)
}

fn default_end() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Serialize, Debug)]
pub struct LokiResponse {
    pub status: String,
    pub data: LogResult,
}

#[derive(Serialize, Debug)]
pub struct LogResult {
    pub result_type: String,
    pub result: Vec<LogEntry>,
}

#[derive(Serialize, Debug)]
pub struct LogEntry {
    pub stream: LogStream,
    pub values: Vec<LogValue>,
}

#[derive(Serialize, Debug)]
pub struct LogStream {
    pub stream: String,
    pub detected_level: String,
    pub namespace: String,
    pub preset_id: Uuid,
    pub project_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Serialize, Debug)]
pub struct LogValue {
    pub value: Vec<[String; 2]>,
}
