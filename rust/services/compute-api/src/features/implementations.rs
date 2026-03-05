use compute_core::models::DeploymentRow;

use crate::features::schemas::{
    DeploymentOut, LogEntry, LogResponse, LokiResponse, LokiTailResponse,
};

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

impl From<DeploymentRow> for DeploymentOut {
    fn from(row: DeploymentRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            project_id: row.project_id,
            preset_id: row.preset_id,
            name: row.name,
            source: row.source.0,
            port: row.port,
            desired_replicas: row.desired_replicas,
            ready_replicas: row.ready_replicas,
            available_replicas: row.available_replicas,
            addon_cpu_millicores: row.addon_cpu_millicores,
            addon_memory_mb: row.addon_memory_mb,
            vault_secret_path: row.vault_secret_path,
            secret_keys: row.secret_keys,
            environment_variables: row.environment_variables.map(|j| j.0).flatten(),
            labels: row.labels.map(|j| j.0).flatten(),
            status: row.status,
            domain: row.domain,
            subdomain: row.subdomain,
            service: row.service,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
