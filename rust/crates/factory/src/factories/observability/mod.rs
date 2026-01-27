pub mod error;
pub mod implementation;

use opentelemetry_sdk::{metrics::SdkMeterProvider, trace::SdkTracerProvider};

pub struct ObservabilityConfig {}

pub struct Observability {
    pub tracer_provider: SdkTracerProvider,
    pub meter_provider: SdkMeterProvider,
}
