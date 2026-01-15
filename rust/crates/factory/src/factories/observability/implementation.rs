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
use time::macros::format_description;
use tracing::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{
    EnvFilter, fmt::time::LocalTime, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::factories::observability::Observability;

impl Drop for Observability {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.meter_provider.shutdown() {
            eprintln!("{err:?}");
        }
    }
}

impl Observability {
    // Initialize tracing-subscriber and return Observability for opentelemetry-related termination processing
    pub async fn init(
        otel_exporter_otlp_endpoint: &str,
        cargo_pkg_name: &str,
        cargo_pkg_version: &str,
    ) -> Observability {
        let resource = Self::get_resource(cargo_pkg_name, cargo_pkg_version);

        let tracer_provider =
            Self::init_tracer_provider(otel_exporter_otlp_endpoint.to_owned(), resource.clone());
        let meter_provider =
            Self::init_meter_provider(otel_exporter_otlp_endpoint.to_string(), resource);

        let tracer = tracer_provider.tracer("tracing-otel-subscriber");
        let open_telemetry_layer = OpenTelemetryLayer::new(tracer);
        let metrics_layer = MetricsLayer::new(meter_provider.clone());

        // Filters
        let level_filter = tracing_subscriber::filter::LevelFilter::from_level(Level::INFO);
        let env_filter = EnvFilter::new(format!(
            "{}=debug,tower_http=warn,hyper=warn,reqwest=warn",
            cargo_pkg_name
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

        Observability {
            tracer_provider,
            meter_provider,
        }
    }

    // Resource
    fn get_resource(cargo_pkg_name: &str, cargo_pkg_version: &str) -> Resource {
        Resource::builder()
            .with_schema_url(
                [
                    KeyValue::new(SERVICE_NAME, cargo_pkg_name.to_string()),
                    KeyValue::new(SERVICE_VERSION, cargo_pkg_version.to_string()),
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
    fn init_meter_provider(
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
}
