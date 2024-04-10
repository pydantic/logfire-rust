use opentelemetry::trace::Tracer;
use opentelemetry_otlp::WithExportConfig;
use std::collections::HashMap;
use std::time::Duration;

static AUTH_HEADER: &'static str = "Authorization";
static TOKEN: &'static str = "wCF04gRksWzKLtvB1j9lqbDcGJwvRn53th7G0TRydsQ0";

#[tokio::main]
async fn main() {
    let headers: HashMap<String, String> =
        HashMap::from([(AUTH_HEADER.to_string(), TOKEN.to_string())]);

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http()
                .with_http_client(reqwest::Client::default())
                .with_headers(headers)
                .with_endpoint("https://api.logfire.dev")
                .with_timeout(Duration::from_secs(5)),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .unwrap();

    tracer.in_span("doing work4", |cx| {
        // tracer.in_span("doing deep work", |_cx| {
        //     // Traced app logic here...
        // });
    });
    let kv = opentelemetry::KeyValue::new("example-attribute", "example-value");

    tracer
        .span_builder("example-span-name2")
        .with_attributes(vec![kv])
        .start(&tracer);
    // dbg!(span);
    // span.finish();

    // Shutdown trace pipeline
    opentelemetry::global::shutdown_tracer_provider();
}
