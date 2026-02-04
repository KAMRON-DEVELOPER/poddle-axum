use crate::features::schemas::{LogEntry, LogResponse, LokiResponse, LokiTailResponse};

impl From<LokiResponse> for LogResponse {
    fn from(loki: LokiResponse) -> Self {
        let mut entries = Vec::new();

        for stream_result in loki.data.result {
            process_stream_result(stream_result, &mut entries);
        }

        LogResponse { entries }
    }
}

impl From<LokiTailResponse> for LogResponse {
    fn from(tail: LokiTailResponse) -> Self {
        let mut entries = Vec::new();
        // The tail API uses 'streams', not 'data.result'
        for stream_result in tail.streams {
            process_stream_result(stream_result, &mut entries);
        }
        LogResponse { entries }
    }
}

fn process_stream_result(
    stream_result: crate::features::schemas::LokiStreamResult,
    entries: &mut Vec<LogEntry>,
) {
    let level = stream_result.stream.get("level").cloned();
    let stream_type = stream_result.stream.get("stream").cloned();

    for value in stream_result.values {
        entries.push(LogEntry {
            timestamp: value[0].clone(),
            message: value[1].clone(),
            level: level.clone(),
            stream: stream_type.clone(),
        });
    }
}
