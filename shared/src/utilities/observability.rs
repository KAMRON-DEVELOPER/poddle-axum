use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use opentelemetry_semantic_conventions::{
    SCHEMA_URL,
    attribute::{SERVICE_NAME, SERVICE_VERSION},
};
use tracing::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::utilities::config::Config;

pub struct OtelGuard {
    pub tracer_provider: SdkTracerProvider,
    pub metric_provider: SdkMeterProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.metric_provider.shutdown() {
            eprintln!("{err:?}");
        }
    }
}

// Create a Resource that captures information about the entity for which telemetry is recorded.
fn get_resource() -> Resource {
    Resource::builder()
        .with_service_name(env!("CARGO_PKG_NAME"))
        .with_schema_url(
            [
                KeyValue::new(SERVICE_NAME, env!("CARGO_CRATE_NAME")),
                KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            ],
            SCHEMA_URL,
        )
        .build()
}

// Construct TracerProvider for OpenTelemetryLayer
fn init_tracer_provider(config: &Config) -> SdkTracerProvider {
    // Initialize OTLP Trace exporter using gRPC (Tonic)
    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint("endpoint")
        .build()
        .expect("Failed to create trace exporter");

    // Create a tracer provider with the exporter
    let tracer_provider = SdkTracerProvider::builder()
        .with_id_generator(RandomIdGenerator::default())
        .with_batch_exporter(trace_exporter)
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(get_resource())
        .build();

    // Set it as the global provider
    global::set_tracer_provider(tracer_provider.clone());

    tracer_provider
}

// Construct MeterProvider for MetricsLayer
fn init_meter_provider(config: &Config) -> SdkMeterProvider {
    // Initialize OTLP Metric exporter using gRPC (Tonic)
    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(config.opt)
        .build()
        .expect("Failed to create metric exporter");

    let reader = PeriodicReader::builder(metric_exporter)
        .with_interval(std::time::Duration::from_secs(30))
        .build();

    // let metric_provider = MeterProviderBuilder::default()
    //     .with_periodic_exporter(metric_exporter)
    //     .with_resource(get_resource())
    //     .with_reader(reader)
    //     .build();

    // Create a metric provider with the OTLP Metric exporter
    let metric_provider = SdkMeterProvider::builder()
        // .with_periodic_exporter(metric_exporter)
        .with_resource(get_resource())
        .with_reader(reader)
        .build();

    global::set_meter_provider(metric_provider.clone());

    metric_provider
}

// Initialize tracing-subscriber and return OtelGuard for opentelemetry-related termination processing
pub fn init_observability(config: &Config) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    let tracer_provider = init_tracer_provider(config);
    let metric_provider = init_meter_provider(config);

    let tracer = tracer_provider.tracer("tracing-otel-subscriber");

    tracing_subscriber::registry()
        // The global level filter prevents the exporter network stack
        // from reentering the globally installed OpenTelemetryLayer with
        // its own spans while exporting, as the libraries should not use
        // tracing levels below DEBUG. If the OpenTelemetry layer needs to
        // trace spans and events with higher verbosity levels, consider using
        // per-layer filtering to target the telemetry layer specifically,
        // e.g. by target matching.
        .with(tracing_subscriber::filter::LevelFilter::from_level(
            Level::INFO,
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(MetricsLayer::new(metric_provider.clone()))
        .with(OpenTelemetryLayer::new(tracer))
        .init();

    Ok(OtelGuard {
        tracer_provider,
        metric_provider,
    })
}
