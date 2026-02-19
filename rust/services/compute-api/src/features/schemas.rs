use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallbackParams {
    pub installation_id: i64,
    pub setup_action: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiResponse {
    pub status: String,
    pub data: LokiData,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LokiData {
    pub result_type: String,
    pub result: Vec<LokiStreamResult>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LokiTailResponse {
    pub streams: Vec<LokiStreamResult>,
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
