use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use opentelemetry_semantic_conventions::{
    SCHEMA_URL,
    attribute::{SERVICE_NAME, SERVICE_VERSION},
};
use time::macros::format_description;
use tonic::transport::ClientTlsConfig;
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
        cargo_crate_name: &str,
        cargo_pkg_version: &str,
        tracing_level: Level,
    ) -> Observability {
        global::set_text_map_propagator(TraceContextPropagator::new());

        let resource = Self::get_resource(cargo_crate_name, cargo_pkg_version);

        let tracer_provider =
            Self::init_tracer_provider(otel_exporter_otlp_endpoint.to_owned(), resource.clone());
        let meter_provider =
            Self::init_meter_provider(otel_exporter_otlp_endpoint.to_string(), resource);

        let tracer = tracer_provider.tracer("tracing-otel-subscriber");
        let open_telemetry_layer = OpenTelemetryLayer::new(tracer);
        let metrics_layer = MetricsLayer::new(meter_provider.clone());

        // Filters
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(tracing_level.as_str().to_lowercase()));

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
    fn get_resource(cargo_crate_name: &str, cargo_pkg_version: &str) -> Resource {
        Resource::builder()
            .with_schema_url(
                [
                    KeyValue::new(SERVICE_NAME, cargo_crate_name.to_string()),
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
        println!("📤 Initializing OTLP trace exporter...");

        let mut exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&otel_exporter_otlp_endpoint)
            .with_compression(opentelemetry_otlp::Compression::Gzip);

        if otel_exporter_otlp_endpoint.starts_with("https://") {
            let tls_config = ClientTlsConfig::new().with_native_roots();
            exporter = exporter.with_tls_config(tls_config);
        }

        // Initialize OTLP Trace exporter using gRPC (Tonic)
        let trace_exporter = exporter.build().expect("Failed to create trace exporter");

        println!("✅ Trace exporter created");

        // Create a tracer provider with the exporter
        let tracer_provider = SdkTracerProvider::builder()
            .with_id_generator(RandomIdGenerator::default())
            .with_batch_exporter(trace_exporter)
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(resource)
            .build();

        // Set it as the global provider
        global::set_tracer_provider(tracer_provider.clone());

        println!("✅ Tracer provider registered globally");

        tracer_provider
    }

    // Construct MeterProvider for MetricsLayer
    fn init_meter_provider(
        otel_exporter_otlp_endpoint: String,
        resource: Resource,
    ) -> SdkMeterProvider {
        println!("📊 Initializing OTLP metric exporter...");

        let mut exporter = MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&otel_exporter_otlp_endpoint)
            .with_compression(opentelemetry_otlp::Compression::Gzip);

        if otel_exporter_otlp_endpoint.starts_with("https://") {
            let tls_config = ClientTlsConfig::new().with_native_roots();
            exporter = exporter.with_tls_config(tls_config);
        }

        // Initialize OTLP Metric exporter using gRPC (Tonic)
        let metric_exporter = exporter.build().expect("Failed to create metric exporter");

        println!("✅ Metric exporter created");

        let reader = PeriodicReader::builder(metric_exporter)
            .with_interval(std::time::Duration::from_secs(30))
            .build();

        // let metric_provider = MeterProviderBuilder::default()
        //     .with_periodic_exporter(metric_exporter)
        //     .with_resource(resource)
        //     .with_reader(reader)
        //     .build();

        // Create a metric provider with the OTLP Metric exporter
        let meter_provider = SdkMeterProvider::builder()
            // .with_periodic_exporter(metric_exporter)
            .with_resource(resource)
            .with_reader(reader)
            .build();

        global::set_meter_provider(meter_provider.clone());

        println!("✅ Meter provider registered globally");

        meter_provider
    }
}
