use opentelemetry::{global, KeyValue};
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::{runtime, Resource};
use opentelemetry_sdk::trace::Config;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub async fn init_otel(oltp_endpoint: &str, pod_name: &str) {
    let resource = Resource::new(vec![KeyValue::new(
        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
        pod_name.to_string(),
    ), KeyValue::new(
        opentelemetry_semantic_conventions::resource::K8S_POD_NAME,
        pod_name.to_string()
    )]);

    let trace_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(oltp_endpoint),
        )
        .with_trace_config(Config::default().with_resource(resource.clone()))
        .install_batch(runtime::Tokio).expect("Failed otel tracer");

    let log_provider = opentelemetry_otlp::new_pipeline()
        .logging()
        .with_resource(resource)

        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(oltp_endpoint),
        )
        .install_batch(runtime::Tokio).expect("");
    // let result = init_tracer_provider(&app_config.otel_collector.oltp_endpoint, resource.clone());

    global::set_text_map_propagator(TraceContextPropagator::new());

    // Create a tracing layer with the configured tracer
    let tracer_layer = tracing_opentelemetry::layer().
        with_tracer(trace_provider.tracer(pod_name.to_string()));

    // Initialize logs and save the logger_provider.
    // let logger_provider = init_logs(&app_config.otel_collector.oltp_endpoint, resource.clone()).unwrap();
    // Create a new OpenTelemetryTracingBridge using the above LoggerProvider.
    let logger_layer = OpenTelemetryTracingBridge::new(&log_provider);

    let filter = EnvFilter::new("info")
        .add_directive("hyper=info".parse().unwrap())
        .add_directive("h2=info".parse().unwrap())
        .add_directive("tonic=info".parse().unwrap())
        .add_directive("reqwest=info".parse().unwrap());

    tracing_subscriber::registry()
        .with(logger_layer)
        .with(filter)
        .with(tracer_layer)
        .init();
    // let meter_provider = init_metrics(&app_config.otel_collector.oltp_endpoint).expect("Failed initializing metrics");
    // global::set_meter_provider(meter_provider);

}

