//! Integration tests for HTTP sink with mock server.

use logfire::config::AdvancedOptions;
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
                                                "unknown_service",
                                            ),
                                        ),
                                    },
                                ),
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
                            },
                            KeyValue {
                                key: "telemetry.sdk.version",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "0.30.0",
                                            ),
                                        ),
                                    },
                                ),
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
                                flags: 1,
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
                                    },
                                    KeyValue {
                                        key: "code.filepath",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "tests/test_http_sink.rs",
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                    KeyValue {
                                        key: "code.lineno",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        55,
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                    KeyValue {
                                        key: "code.namespace",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_http_sink",
                                                    ),
                                                ),
                                            },
                                        ),
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
                                    },
                                    KeyValue {
                                        key: "logfire.span_type",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "span",
                                                    ),
                                                ),
                                            },
                                        ),
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
                                                "unknown_service",
                                            ),
                                        ),
                                    },
                                ),
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
                            },
                            KeyValue {
                                key: "telemetry.sdk.version",
                                value: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "0.30.0",
                                            ),
                                        ),
                                    },
                                ),
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
                                flags: 1,
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
                                    },
                                    KeyValue {
                                        key: "code.filepath",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "tests/test_http_sink.rs",
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                    KeyValue {
                                        key: "code.lineno",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        370,
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                    KeyValue {
                                        key: "code.namespace",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "test_http_sink",
                                                    ),
                                                ),
                                            },
                                        ),
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
                                    },
                                    KeyValue {
                                        key: "logfire.span_type",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "span",
                                                    ),
                                                ),
                                            },
                                        ),
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

/// Mutex to avoid concurrent mutation of env vars.
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
