use crate::utilities::config::Config;
use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use opentelemetry_semantic_conventions::{
    SCHEMA_URL,
    attribute::{DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_NAME, SERVICE_VERSION},
};
use time::macros::format_description;
use tracing::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{
    EnvFilter, fmt::time::LocalTime, layer::SubscriberExt, util::SubscriberInitExt,
};

pub struct OtelGuard {
    pub tracer_provider: SdkTracerProvider,
    pub meter_provider: SdkMeterProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.meter_provider.shutdown() {
            eprintln!("{err:?}");
        }
    }
}

// Create a Resource that captures information about the entity for which telemetry is recorded.
fn get_resource(config: &Config) -> Resource {
    Resource::builder()
        .with_schema_url(
            [
                KeyValue::new(SERVICE_NAME, config.cargo_pkg_name.clone()),
                KeyValue::new(SERVICE_VERSION, config.cargo_pkg_version.clone()),
                KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, config.environment.clone()),
            ],
            SCHEMA_URL,
        )
        .build()
}

// Construct TracerProvider for OpenTelemetryLayer
fn init_tracer_provider(
    otel_exporter_otlp_endpoint: String,
    resource: Resource,
) -> SdkTracerProvider {
    // Initialize OTLP Trace exporter using gRPC (Tonic)
    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otel_exporter_otlp_endpoint)
        .build()
        .expect("Failed to create trace exporter");

    // Create a tracer provider with the exporter
    let tracer_provider = SdkTracerProvider::builder()
        .with_id_generator(RandomIdGenerator::default())
        .with_batch_exporter(trace_exporter)
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(resource)
        .build();

    // Set it as the global provider
    global::set_tracer_provider(tracer_provider.clone());

    tracer_provider
}

// Construct MeterProvider for MetricsLayer
fn init_metric_provider(
    otel_exporter_otlp_endpoint: String,
    resource: Resource,
) -> SdkMeterProvider {
    // Initialize OTLP Metric exporter using gRPC (Tonic)
    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otel_exporter_otlp_endpoint)
        .build()
        .expect("Failed to create metric exporter");

    let reader = PeriodicReader::builder(metric_exporter)
        .with_interval(std::time::Duration::from_secs(30))
        .build();

    // let metric_provider = MeterProviderBuilder::default()
    //     .with_periodic_exporter(metric_exporter)
    //     .with_resource(resource)
    //     .with_reader(reader)
    //     .build();

    // Create a metric provider with the OTLP Metric exporter
    let metric_provider = SdkMeterProvider::builder()
        // .with_periodic_exporter(metric_exporter)
        .with_resource(resource)
        .with_reader(reader)
        .build();

    global::set_meter_provider(metric_provider.clone());

    metric_provider
}

// Initialize tracing-subscriber and return OtelGuard for opentelemetry-related termination processing
pub async fn init_observability(config: &Config) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    let resource = get_resource(config);

    let tracer_provider =
        init_tracer_provider(config.otel_exporter_otlp_endpoint.clone(), resource.clone());
    let meter_provider = init_metric_provider(config.otel_exporter_otlp_endpoint.clone(), resource);

    let tracer = tracer_provider.tracer("tracing-otel-subscriber");
    let open_telemetry_layer = OpenTelemetryLayer::new(tracer);
    let metrics_layer = MetricsLayer::new(meter_provider.clone());

    // Filters
    let level_filter = tracing_subscriber::filter::LevelFilter::from_level(Level::INFO);
    let env_filter = EnvFilter::new(format!(
        "{}=debug,shared=debug,tower_http=warn,hyper=warn,reqwest=warn",
        config.cargo_pkg_name
    ));

    // Stdout
    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second]"
    ));
    let fmt_layer = tracing_subscriber::fmt::layer()
        // Use a more compact, abbreviated log format
        .compact()
        // Don't display the event's target (module path)
        .with_target(false)
        // Display source code file paths
        .with_file(true)
        // Display source code line numbers
        .with_line_number(true)
        .with_timer(timer)
        .json();

    // Registry
    tracing_subscriber::registry()
        .with(level_filter)
        .with(env_filter)
        .with(fmt_layer)
        .with(metrics_layer)
        .with(open_telemetry_layer)
        .init();

    Ok(OtelGuard {
        tracer_provider,
        meter_provider,
    })
}
