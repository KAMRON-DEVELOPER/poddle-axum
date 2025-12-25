pub mod implementations;
pub mod models;
pub mod schemas;
pub mod services;
pub mod utilities;

pub fn init_telemetry(service_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(
            opentelemetry_sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service_name.to_owned(),
            )])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(tracing_subscriber::fmt::layer().json()) // For Loki/Fluentd
        .with(EnvFilter::from_default_env())
        .init();

    Ok(())
}
