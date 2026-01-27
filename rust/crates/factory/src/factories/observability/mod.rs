pub mod error;
pub mod implementation;

use opentelemetry_sdk::{metrics::SdkMeterProvider, trace::SdkTracerProvider};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct ObservabilityConfig {
    pub otel_exporter_otlp_endpoint: String,
    pub rust_log: Option<String>,
    pub log_format: Option<String>,
    pub tracing_level: Option<String>,
    pub with_file: Option<bool>,
    pub with_line_number: Option<bool>,
}

pub struct Observability {
    pub tracer_provider: SdkTracerProvider,
    pub meter_provider: SdkMeterProvider,
}
