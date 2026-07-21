//! Integration tests for HTTP sink with mock server.

use logfire::config::{AdvancedOptions, MetricsOptions};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use insta::assert_debug_snapshot;
use logfire::{configure, set_local_logfire, span};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;

use crate::test_utils::{DeterministicIdGenerator, make_trace_request_deterministic};

#[path = "../src/test_utils.rs"]
mod test_utils;

/// Test HTTP protobuf export infrastructure
#[tokio::test]
async fn test_http_protobuf_export() {
    let mock_server = MockServer::start().await;

    let traces_mock = Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .and(header("content-type", "application/x-protobuf"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200));

    mock_server.register(traces_mock).await;

    let env_guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: Holding mutex to prevent other thread interacting with env
    unsafe {
        std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf");
    }

    let logfire = configure()
        .local()
        .send_to_logfire(true)
        .with_token("test-token")
        .with_advanced_options(
            AdvancedOptions::default()
                .with_base_url(mock_server.uri())
                .with_id_generator(DeterministicIdGenerator::new()),
        )
        .finish()
        .unwrap();

    // SAFETY: As above
    unsafe {
        std::env::remove_var("OTEL_EXPORTER_OTLP_PROTOCOL");
    }
    drop(env_guard);

    let guard = set_local_logfire(logfire);
    span!("test_span").in_scope(|| {});
    guard.shutdown().unwrap();

    let requests = mock_server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];

    assert_eq!(request.method.as_str(), "POST");
    assert_eq!(request.url.path(), "/v1/traces");
    assert_eq!(
        request.headers.get("user-agent").expect("no user-agent"),
        &format!("logfire-rust/{}", env!("CARGO_PKG_VERSION"))
    );
    let mut body = ExportTraceServiceRequest::decode(request.body.as_slice())
        .expect("failed to decode trace request");
    make_trace_request_deterministic(&mut body);

    assert_debug_snapshot!(body, @r#"
    ExportTraceServiceRequest {
        resource_spans: [
            ResourceSpans {
                resource: Some(
                    Resource {
                        attributes: [
                            KeyValue {
                                key: "service.name",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "unknown_service:<process name>",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                            KeyValue {
                                key: "telemetry.sdk.language",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "rust",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                            KeyValue {
                                key: "telemetry.sdk.name",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "opentelemetry",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                            KeyValue {
                                key: "telemetry.sdk.version",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "0.0.0",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                        ],
                        dropped_attributes_count: 0,
                        entity_refs: [],
                    },
                ),
                scope_spans: [
                    ScopeSpans {
                        scope: Some(
                            InstrumentationScope {
                                name: "logfire",
                                version: "",
                                attributes: [],
                                dropped_attributes_count: 0,
                            },
                        ),
                        spans: [
                            Span {
                                trace_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    240,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    240,
                                ],
                                trace_state: "",
                                parent_span_id: [],
                                flags: 257,
                                name: "test_span",
                                kind: Internal,
                                start_time_unix_nano: 0,
                                end_time_unix_nano: 1000000000,
                                attributes: [
                                    KeyValue {
                                        key: "busy_ns",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        0,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "code.file.path",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "logfire/tests/test_http_sink.rs",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "code.line.number",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        55,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "code.module.name",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_http_sink",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "idle_ns",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        0,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "logfire.json_schema",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "{\"type\":\"object\",\"properties\":{}}",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "logfire.level_num",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        9,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "logfire.msg",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_span",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "thread.id",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        0,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "thread.name",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_http_protobuf_export",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                ],
                                dropped_attributes_count: 0,
                                events: [],
                                dropped_events_count: 0,
                                links: [],
                                dropped_links_count: 0,
                                status: Some(
                                    Status {
                                        message: "",
                                        code: Unset,
                                    },
                                ),
                            },
                        ],
                        schema_url: "",
                    },
                ],
                schema_url: "",
            },
        ],
    }
    "#);
}

/// Test HTTP JSON export to mock server
#[tokio::test]
async fn test_http_json_export() {
    let mock_server = MockServer::start().await;

    let traces_mock = Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .and(header("content-type", "application/json"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200));

    mock_server.register(traces_mock).await;

    let env_guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: Holding mutex to prevent other thread interacting with env
    unsafe {
        std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json");
    }

    let logfire = configure()
        .local()
        .send_to_logfire(true)
        .with_token("test-token")
        .with_advanced_options(
            AdvancedOptions::default()
                .with_base_url(mock_server.uri())
                .with_id_generator(DeterministicIdGenerator::new()),
        )
        .finish()
        .unwrap();

    // SAFETY: As above
    unsafe {
        std::env::remove_var("OTEL_EXPORTER_OTLP_PROTOCOL");
    }
    drop(env_guard);

    let guard = set_local_logfire(logfire);
    span!("json_test_span").in_scope(|| {});
    guard.shutdown().unwrap();

    let requests = mock_server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];

    assert_eq!(request.method.as_str(), "POST");
    assert_eq!(request.url.path(), "/v1/traces");
    assert_eq!(
        request.headers.get("user-agent").expect("no user-agent"),
        &format!("logfire-rust/{}", env!("CARGO_PKG_VERSION"))
    );
    let mut body: ExportTraceServiceRequest =
        serde_json::from_slice(&request.body).expect("failed to decode trace request");

    make_trace_request_deterministic(&mut body);

    assert_debug_snapshot!(body, @r#"
    ExportTraceServiceRequest {
        resource_spans: [
            ResourceSpans {
                resource: Some(
                    Resource {
                        attributes: [
                            KeyValue {
                                key: "service.name",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "unknown_service:<process name>",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                            KeyValue {
                                key: "telemetry.sdk.language",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "rust",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                            KeyValue {
                                key: "telemetry.sdk.name",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "opentelemetry",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                            KeyValue {
                                key: "telemetry.sdk.version",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "0.0.0",
                                            ),
                                        ),
                                    },
                                ),
                                key_strindex: 0,
                            },
                        ],
                        dropped_attributes_count: 0,
                        entity_refs: [],
                    },
                ),
                scope_spans: [
                    ScopeSpans {
                        scope: Some(
                            InstrumentationScope {
                                name: "logfire",
                                version: "",
                                attributes: [],
                                dropped_attributes_count: 0,
                            },
                        ),
                        spans: [
                            Span {
                                trace_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    240,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    240,
                                ],
                                trace_state: "",
                                parent_span_id: [],
                                flags: 257,
                                name: "json_test_span",
                                kind: Internal,
                                start_time_unix_nano: 0,
                                end_time_unix_nano: 1000000000,
                                attributes: [
                                    KeyValue {
                                        key: "busy_ns",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        0,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "code.file.path",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "logfire/tests/test_http_sink.rs",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "code.line.number",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        376,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "code.module.name",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_http_sink",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "idle_ns",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        0,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "logfire.json_schema",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "{\"type\":\"object\",\"properties\":{}}",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "logfire.level_num",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        9,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "logfire.msg",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "json_test_span",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "thread.id",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        0,
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                    KeyValue {
                                        key: "thread.name",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_http_json_export",
                                                    ),
                                                ),
                                            },
                                        ),
                                        key_strindex: 0,
                                    },
                                ],
                                dropped_attributes_count: 0,
                                events: [],
                                dropped_events_count: 0,
                                links: [],
                                dropped_links_count: 0,
                                status: Some(
                                    Status {
                                        message: "",
                                        code: Unset,
                                    },
                                ),
                            },
                        ],
                        schema_url: "",
                    },
                ],
                schema_url: "",
            },
        ],
    }
    "#);
}

/// Test that metrics are exported over HTTP with the logfire user agent.
///
/// Runs for both HTTP protocols, as they are configured by separate code paths.
#[tokio::test(flavor = "multi_thread")]
async fn test_http_metrics_export() {
    for protocol in ["http/protobuf", "http/json"] {
        let mock_server = MockServer::start().await;

        mock_server
            .register(Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)))
            .await;

        let env_guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: Holding mutex to prevent other thread interacting with env
        unsafe {
            std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", protocol);
        }

        let logfire = configure()
            .local()
            .send_to_logfire(true)
            .with_token("test-token")
            .with_metrics(Some(MetricsOptions::default()))
            .with_advanced_options(AdvancedOptions::default().with_base_url(mock_server.uri()))
            .finish()
            .unwrap();

        // SAFETY: As above
        unsafe {
            std::env::remove_var("OTEL_EXPORTER_OTLP_PROTOCOL");
        }
        drop(env_guard);

        logfire.metrics().u64_counter("test_counter").build().add(
            1,
            &[opentelemetry::KeyValue::new("test_attribute", "value")],
        );
        tokio::task::spawn_blocking(move || logfire.shutdown())
            .await
            .unwrap()
            .unwrap();

        let requests = mock_server.received_requests().await.unwrap();
        let metrics_requests: Vec<_> = requests
            .iter()
            .filter(|request| request.url.path() == "/v1/metrics")
            .collect();
        assert!(
            !metrics_requests.is_empty(),
            "no metrics exported for {protocol}"
        );
        for request in metrics_requests {
            assert_eq!(
                request.headers.get("user-agent").expect("no user-agent"),
                &format!("logfire-rust/{}", env!("CARGO_PKG_VERSION"))
            );
        }
    }
}

/// An unsupported protocol in the environment is an error.
#[test]
fn test_unsupported_protocol() {
    let env_guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: Holding mutex to prevent other thread interacting with env
    unsafe {
        std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", "carrier-pigeon");
    }

    let error = logfire::exporters::span_exporter("http://localhost:4318", None)
        .err()
        .expect("expected an error for an unsupported protocol");

    // SAFETY: As above
    unsafe {
        std::env::remove_var("OTEL_EXPORTER_OTLP_PROTOCOL");
    }
    drop(env_guard);

    assert_eq!(error.to_string(), "unsupported protocol: carrier-pigeon");
}

/// Mutex to avoid concurrent mutation of env vars.
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
