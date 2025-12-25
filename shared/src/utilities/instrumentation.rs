use tracing_subscriber::EnvFilter;

pub fn init_telemetry(service_name: &str) {
    // 1. Create a tracer that exports to the OTel Collector (usually running in K8s)
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://otel-collector:4317"),
        )
        .with_trace_config(
            opentelemetry_sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service_name.to_string(),
            )])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("failed to install tracer");

    // 2. Create a telemetry layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // 3. Create your existing fmt layer (for stdout/kubectl logs)
    let fmt_layer = tracing_subscriber::fmt::layer().json(); // JSON is better for Promtail/Loki parsing

    // 4. Combine them
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt_layer)
        .with(telemetry)
        .init();
}
