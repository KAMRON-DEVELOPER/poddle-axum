use crate::features::schemas::{LogEntry, LogResponse, LokiResponse};

impl From<LokiResponse> for LogResponse {
    fn from(loki: LokiResponse) -> Self {
        let mut entries = Vec::new();

        for stream_result in loki.data.result {
            // Extract common labels for this batch
            let level = stream_result.stream.get("level").cloned();
            let stream_type = stream_result.stream.get("stream").cloned(); // stdout/stderr

            for value in stream_result.values {
                // value is [timestamp, log_line]
                entries.push(LogEntry {
                    timestamp: value[0].clone(),
                    message: value[1].clone(),
                    level: level.clone(),
                    stream: stream_type.clone(),
                });
            }
        }

        LogResponse { entries }
    }
}
