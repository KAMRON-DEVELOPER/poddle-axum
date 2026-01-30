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
    pub start: Option<String>,
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

// ----------------------------------------
//
// ----------------------------------------
// ----------------------------------------
//
// ----------------------------------------

// // What gets sent to Loki
// #[derive(Debug, Serialize)]
// pub struct LogEntry {
//     // The actual log line (original or from .Entry)
//     pub message: String,

//     // Labels (indexed)
//     pub labels: LogLabels,

//     // Structured metadata (not indexed, but queryable)
//     pub metadata: LogMetadata,
// }

#[derive(Debug, Serialize)]
pub struct LogLabels {
    // From discovery/relabel
    pub job: String, // "namespace/pod_name"
    pub namespace: String,
    pub pod: String,
    pub container: String,
    pub app: Option<String>,
    pub project_id: Option<String>,
    pub deployment_id: Option<String>,
    pub managed_by: Option<String>,

    // From CRI parsing
    pub stream: String, // "stdout" or "stderr"
    pub flags: String,  // "F" or "P"

    // From log parsing
    pub level: Option<String>, // "info", "error", etc.
}

#[derive(Debug, Serialize)]
pub struct LogMetadata {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,

    // These normalized fields exist in the pipeline
    // but may not be sent as metadata
    pub normalized_message: Option<String>,
    pub normalized_timestamp: Option<String>,
}

// ----------------------------------------
//
// ----------------------------------------
// ----------------------------------------
//
// ----------------------------------------

// // Matches what you send to the React frontend
// #[derive(Serialize, Deserialize, Debug)]
// pub struct LogEntry {
//     pub timestamp: String,      // From Loki value[0]
//     pub message: String,        // The cleaned log line from stage.output
//     pub level: Option<String>,  // Extracted from Loki labels
//     pub stream: Option<String>, // "stdout" or "stderr"
// }

// // Internal representation for parsing Loki's JSON response
// #[derive(Deserialize, Debug)]
// pub struct LokiStream {
//     pub stream: HashMap<String, String>, // Labels: project_id, level, pod, etc.
//     pub values: Vec<[String; 2]>,        // [timestamp_ns, log_line]
// }
