use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use compute_core::schemas::ProjectPageQuery;
use http_contracts::pagination::schema::Pagination;
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
pub struct TailQuery {
    #[serde(default = "default_start")]
    pub start: DateTime<Utc>,
}

fn default_start() -> DateTime<Utc> {
    Utc::now() - Duration::minutes(15)
}

fn default_end() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiResponse {
    pub status: String,
    pub data: LokiData,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiTailResponse {
    pub streams: Vec<LokiStreamResult>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiData {
    #[serde(rename = "resultType")]
    pub result_type: String,
    pub result: Vec<LokiStreamResult>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiStreamResult {
    pub stream: HashMap<String, String>,
    pub values: Vec<[String; 2]>,
}

#[derive(Serialize, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: Option<String>,
    pub stream: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct LogResponse {
    pub entries: Vec<LogEntry>,
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LogStreamEvent {
    LogBatch { entries: Vec<LogEntry> },
}

// #[derive(Deserialize, Serialize, Debug)]
// pub struct LokiStreamResult {
//     pub stream: LokiStream,
//     pub values: Vec<[String; 2]>,
// }

// #[derive(Deserialize, Serialize, Debug)]
// pub struct LokiStream {
//     pub stream: String,
//     pub detected_level: String,
//     pub namespace: String,
//     pub preset_id: Uuid,
//     pub project_id: Uuid,
//     pub deployment_id: Uuid,
// }
