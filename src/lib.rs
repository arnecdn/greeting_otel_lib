use std::sync::OnceLock;
use opentelemetry::{global, KeyValue};
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::{runtime, Resource};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::{Config, SdkTracerProvider};
use tracing_subscriber::{EnvFilter, Layer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn get_resource(resource_name:&str, pod_name:&str) -> Resource {
    static RESOURCE: OnceLock<Resource> = OnceLock::new();
    RESOURCE
        .get_or_init(|| {
            Resource::builder()
                .with_service_name(resource_name.to_string())
                .with_attribute(
                    KeyValue::new(
                        opentelemetry_semantic_conventions::resource::K8S_POD_NAME,
                        pod_name.to_string()
                ))
                .build()
        })
        .clone()
}

fn init_traces(oltp_endpoint: &str, app_name: &str, pod_name: &str) -> SdkTracerProvider {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(oltp_endpoint)
        .build()
        .expect("Failed to create span exporter");
    SdkTracerProvider::builder()
        .with_resource(get_resource(app_name, pod_name))
        .with_batch_exporter(exporter)
        .build()
}

fn init_metrics(oltp_endpoint: &str, app_name: &str, pod_name: &str) -> SdkMeterProvider {
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(oltp_endpoint)
        .build()
        .expect("Failed to create metric exporter");

    SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(get_resource(app_name, pod_name))
        .build()
}

fn init_logs(oltp_endpoint: &str, app_name: &str, pod_name: &str) -> SdkLoggerProvider {
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(oltp_endpoint)
        .build()
        .expect("Failed to create log exporter");

    SdkLoggerProvider::builder()
        .with_resource(get_resource(app_name, pod_name))
        .with_batch_exporter(exporter)
        .build()
}

pub async fn init_otel(oltp_endpoint: &str, app_name: &str, pod_name: &str) {
    let logger_provider = init_logs(oltp_endpoint,app_name, pod_name);

    // Create a new OpenTelemetryTracingBridge using the above LoggerProvider.
    let otel_layer = OpenTelemetryTracingBridge::new(&logger_provider);

    // For the OpenTelemetry layer, add a tracing filter to filter events from
    // OpenTelemetry and its dependent crates (opentelemetry-otlp uses crates
    // like reqwest/tonic etc.) from being sent back to OTel itself, thus
    // preventing infinite telemetry generation. The filter levels are set as
    // follows:
    // - Allow `info` level and above by default.
    // - Restrict `opentelemetry`, `hyper`, `tonic`, and `reqwest` completely.
    // Note: This will also drop events from crates like `tonic` etc. even when
    // they are used outside the OTLP Exporter. For more details, see:
    // https://github.com/open-telemetry/opentelemetry-rust/issues/761
    let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("opentelemetry=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());
    let otel_layer = otel_layer.with_filter(filter_otel);

    // Create a new tracing::Fmt layer to print the logs to stdout. It has a
    // default filter of `info` level and above, and `debug` and above for logs
    // from OpenTelemetry crates. The filter levels can be customized as needed.
    let filter_fmt = EnvFilter::new("info").add_directive("opentelemetry=debug".parse().unwrap());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(true)
        .with_filter(filter_fmt);

    // Initialize the tracing subscriber with the OpenTelemetry layer and the
    // Fmt layer.
    tracing_subscriber::registry()
        .with(otel_layer)
        .with(fmt_layer)
        .init();

    // At this point Logs (OTel Logs and Fmt Logs) are initialized, which will
    // allow internal-logs from Tracing/Metrics initializer to be captured.

    let tracer_provider = init_traces(oltp_endpoint,app_name, pod_name);
    // Set the global tracer provider using a clone of the tracer_provider.
    // Setting global tracer provider is required if other parts of the application
    // uses global::tracer() or global::tracer_with_version() to get a tracer.
    // Cloning simply creates a new reference to the same tracer provider. It is
    // important to hold on to the tracer_provider here, so as to invoke
    // shutdown on it when application ends.
    global::set_tracer_provider(tracer_provider.clone());

    global::set_text_map_propagator(TraceContextPropagator::new());

    // let meter_provider = init_metrics(&app_config.otel_collector.oltp_endpoint).expect("Failed initializing metrics");
    // global::set_meter_provider(meter_provider);

}

