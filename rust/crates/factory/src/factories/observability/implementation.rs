use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use opentelemetry_semantic_conventions::{SCHEMA_URL, attribute::SERVICE_VERSION};
use time::macros::format_description;
use tonic::transport::ClientTlsConfig;
use tracing::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{
    EnvFilter, Layer, fmt::time::LocalTime, layer::SubscriberExt, util::SubscriberInitExt,
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
    /// Initialize tracing-subscriber and return Observability for opentelemetry-related termination processing.
    pub async fn init(
        otel_exporter_otlp_endpoint: &str,
        cargo_crate_name: &str,
        cargo_pkg_version: &str,
        rust_log: Option<&str>,
        log_format: Option<&str>,
        tracing_level: Option<&str>,
        with_file: Option<bool>,
        with_line_number: Option<bool>,
    ) -> Observability {
        global::set_text_map_propagator(TraceContextPropagator::new());

        let resource = Self::get_resource(cargo_crate_name, cargo_pkg_version);

        let tracer_provider =
            Self::init_tracer_provider(resource.clone(), otel_exporter_otlp_endpoint.to_owned());
        let meter_provider =
            Self::init_meter_provider(resource, otel_exporter_otlp_endpoint.to_string());

        let tracer = tracer_provider.tracer("tracing-otel-subscriber");
        let open_telemetry_layer = OpenTelemetryLayer::new(tracer);
        let metrics_layer = MetricsLayer::new(meter_provider.clone());

        // Filters
        let level = match tracing_level {
            None => Level::INFO,
            Some("TRACE") => Level::TRACE,
            Some("DEBUG") => Level::DEBUG,
            Some("INFO") => Level::INFO,
            Some("WARN") => Level::WARN,
            Some("ERROR") => Level::ERROR,
            _ => Level::INFO,
        };

        let env_filter = match rust_log {
            Some(rust_log) => EnvFilter::new(rust_log),
            None => EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(level.as_str().to_lowercase())),
        };

        // Stdout
        let log_format = log_format.unwrap_or("json");
        let timer = LocalTime::new(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second]"
        ));

        let fmt_layer = if log_format == "pretty" {
            tracing_subscriber::fmt::layer()
                .with_ansi(true)
                .with_timer(timer)
                .with_target(false)
                .with_file(with_file.unwrap_or(true))
                .with_line_number(with_line_number.unwrap_or(true))
                .compact()
                .boxed()
        } else {
            tracing_subscriber::fmt::layer()
                .with_timer(timer)
                .with_target(false)
                .with_file(with_file.unwrap_or(true))
                .with_line_number(with_line_number.unwrap_or(true))
                .json()
                .flatten_event(true)
                .with_span_list(false)
                .boxed()
        };

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
            .with_service_name(cargo_crate_name.to_string())
            .with_schema_url(
                [KeyValue::new(
                    SERVICE_VERSION,
                    cargo_pkg_version.to_string(),
                )],
                SCHEMA_URL,
            )
            .build()
    }

    // Construct TracerProvider for OpenTelemetryLayer
    fn init_tracer_provider(
        resource: Resource,
        otel_exporter_otlp_endpoint: String,
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
        resource: Resource,
        otel_exporter_otlp_endpoint: String,
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
