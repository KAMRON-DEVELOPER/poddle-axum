use std::env;

use opentelemetry_sdk::Resource;

pub struct ObservabilityConfig {
    pub service_name: String,
    pub service_version: String,
    pub otlp_endpoint: String,
    pub environment: String,
    pub log_level: String,
}

impl ObservabilityConfig {
    pub fn from_env(service_name: &str) -> Result<Self, std::env::VarError> {
        Ok(Self {
            service_name: service_name.to_string(),
            service_version: env::var("SERVICE_VERSION").unwrap_or_else(|_| "0.1.0".to_string()),
            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            log_level: env::var("RUST_LOG")
                .unwrap_or_else(|_| format!("{}=debug,shared=debug", service_name)),
        })
    }
}

pub fn init_observability(config: ObservabilityConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create resource with service metadata
    let resource = Resource::new(vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.version", config.service_version),
        KeyValue::new("deployment.environment", config.environment),
    ]);

    // Configure OpenTelemetry tracer
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(&config.otlp_endpoint),
        )
        .with_trace_config(
            trace::Config::default()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    1.0,
                ))))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    // Create telemetry layer
    let telemetry_layer =
        tracing_opentelemetry::layer().with_tracer(tracer.tracer(config.service_name.clone()));

    // Environment filter for logs
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    // Console formatting layer (for local development)
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_filter(env_filter);

    // Initialize subscriber with multiple layers
    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(fmt_layer)
        .init();

    Ok(())
}

pub fn shutdown_observability() {
    global::shutdown_tracer_provider();
}

// Instrumentation helper macros
#[macro_export]
macro_rules! trace_error {
    ($err:expr) => {{
        let error = &$err;
        tracing::error!(
            error = %error,
            error_type = std::any::type_name_of_val(error),
            "Error occurred"
        );
        error
    }};
}

pub trait ErrorTracing {
    fn trace_err(self) -> Self;
}

impl<T, E: std::fmt::Display> ErrorTracing for Result<T, E> {
    fn trace_err(self) -> Self {
        if let Err(ref e) = self {
            tracing::error!(error = %e, "Operation failed");
        }
        self
    }
}
