use std::collections::HashMap;

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
    pub start_time: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct LogEntry {
    pub message: String,
    pub level: Option<String>,
    pub timestamp: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiResult {
    pub stream: HashMap<String, String>,
    pub values: Vec<(String, String)>,
}

#[derive(Deserialize, Debug)]
pub struct LokiResponse {
    pub status: String,
    pub data: LokiData,
}

#[derive(Deserialize, Debug)]
pub struct LokiData {
    pub result_type: String,
    pub result: Vec<LokiStream>,
}

#[derive(Deserialize, Debug)]
pub struct LokiStream {
    pub stream: HashMap<String, String>,
    pub values: Vec<[String; 2]>,
}
