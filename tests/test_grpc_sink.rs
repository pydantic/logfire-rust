//! Integration tests for gRPC sink with mock server.
#![cfg(feature = "export-grpc")]

use insta::assert_debug_snapshot;
use logfire::{configure, info, set_local_logfire, span};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
    logs_service_server::{LogsService, LogsServiceServer},
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
    trace_service_server::{TraceService, TraceServiceServer},
};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tonic::{Request, Response, Status, transport::Server};

use crate::test_utils::{DeterministicIdGenerator, make_trace_request_deterministic};

#[path = "../src/test_utils.rs"]
mod test_utils;

/// Mock trace service that captures requests for testing
#[derive(Clone)]
struct MockTraceService {
    captured: Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
}

impl MockTraceService {
    fn new(captured: Arc<Mutex<Vec<ExportTraceServiceRequest>>>) -> Self {
        Self { captured }
    }
}

/// Mock logs service that captures requests for testing
#[derive(Clone)]
struct MockLogsService {
    captured: Arc<Mutex<Vec<ExportLogsServiceRequest>>>,
}

impl MockLogsService {
    fn new(captured: Arc<Mutex<Vec<ExportLogsServiceRequest>>>) -> Self {
        Self { captured }
    }
}

#[tonic::async_trait]
impl TraceService for MockTraceService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        let trace_request = request.into_inner();

        // Capture the request for testing
        {
            let mut captured = self.captured.lock().unwrap();
            captured.push(trace_request);
        }

        Ok(Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

#[tonic::async_trait]
impl LogsService for MockLogsService {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        let logs_request = request.into_inner();

        // Capture the request for testing
        {
            let mut captured = self.captured.lock().unwrap();
            captured.push(logs_request);
        }

        Ok(Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

/// Start a mock gRPC server and return its address
async fn start_mock_grpc_server() -> (
    String,
    Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
    Arc<Mutex<Vec<ExportLogsServiceRequest>>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let addr_str = format!("http://{}", addr);

    let captured_traces = Arc::new(Mutex::new(Vec::new()));
    let captured_traces_clone = captured_traces.clone();

    let captured_logs = Arc::new(Mutex::new(Vec::new()));
    let captured_logs_clone = captured_logs.clone();

    let trace_service = MockTraceService::new(captured_traces_clone);
    let logs_service = MockLogsService::new(captured_logs_clone);

    tokio::spawn(async move {
        Server::builder()
            .add_service(TraceServiceServer::new(trace_service))
            .add_service(LogsServiceServer::new(logs_service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .expect("gRPC server failed to start");
    });

    let channel = tonic::transport::Channel::from_shared(addr_str.clone())
        .unwrap()
        .connect()
        .await
        .expect("Failed to connect to mock gRPC server");

    tonic::client::Grpc::new(channel).ready().await.unwrap();

    (addr_str, captured_traces, captured_logs)
}

/// Test gRPC protobuf export infrastructure
#[tokio::test]
async fn test_grpc_protobuf_export() {
    use logfire::config::AdvancedOptions;

    let (server_addr, captured_traces, captured_logs) = start_mock_grpc_server().await;

    let env_guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: Holding mutex to prevent other threads interacting with env
    unsafe {
        std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc");
    }

    let logfire = configure()
        .local()
        .send_to_logfire(true)
        .with_token("test-token")
        .with_advanced_options(
            AdvancedOptions::default()
                .with_base_url(server_addr)
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
    span!("grpc_test_span").in_scope(|| {
        info!("Test log message from within span");
    });
    info!("Test log message outside span");
    tokio::task::spawn_blocking(move || guard.shutdown())
        .await
        .unwrap()
        .unwrap();

    let trace_requests = captured_traces.lock().unwrap();
    let log_requests = captured_logs.lock().unwrap();

    assert_eq!(trace_requests.len(), 1);
    assert!(!log_requests.is_empty()); // At least one log request

    let mut trace_body = trace_requests[0].clone();
    make_trace_request_deterministic(&mut trace_body);

    assert_debug_snapshot!(trace_body, @r#"
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
                                    241,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    243,
                                ],
                                trace_state: "",
                                parent_span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    242,
                                ],
                                flags: 1,
                                name: "poll",
                                kind: Internal,
                                start_time_unix_nano: 0,
                                end_time_unix_nano: 0,
                                attributes: [
                                    KeyValue {
                                        key: "code.filepath",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "/home/david/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/h2-0.4.12/src/proto/connection.rs",
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
                                                        269,
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
                                                        "h2::proto::connection",
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
                                                        1,
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
                                                        "pending_span",
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
                                                        "test_grpc_protobuf_export",
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
                                    241,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    244,
                                ],
                                trace_state: "",
                                parent_span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    242,
                                ],
                                flags: 1,
                                name: "poll_ready",
                                kind: Internal,
                                start_time_unix_nano: 1000000000,
                                end_time_unix_nano: 2000000000,
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
                                                        "/home/david/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/h2-0.4.12/src/proto/connection.rs",
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
                                                        188,
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
                                                        "h2::proto::connection",
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
                                        key: "logfire.level_num",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        1,
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
                                                        "test_grpc_protobuf_export",
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
                                name: "grpc_test_span",
                                kind: Internal,
                                start_time_unix_nano: 3000000000,
                                end_time_unix_nano: 4000000000,
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
                                                        "tests/test_grpc_sink.rs",
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
                                                        158,
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
                                                        "test_grpc_sink",
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
                                                        "grpc_test_span",
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
                                                        "test_grpc_protobuf_export",
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

    assert_eq!(log_requests.len(), 1);

    let mut log_body = log_requests[0].clone();

    // TODO deterministic

    assert_debug_snapshot!(log_body, @r#"
    ExportLogsServiceRequest {
        resource_logs: [
            ResourceLogs {
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
                scope_logs: [
                    ScopeLogs {
                        scope: Some(
                            InstrumentationScope {
                                name: "logfire",
                                version: "",
                                attributes: [],
                                dropped_attributes_count: 0,
                            },
                        ),
                        log_records: [
                            LogRecord {
                                time_unix_nano: 1755687117943699156,
                                observed_time_unix_nano: 1755687117943699156,
                                severity_number: Info,
                                severity_text: "INFO",
                                body: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "Test log message from within span",
                                            ),
                                        ),
                                    },
                                ),
                                attributes: [
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
                                        key: "thread.id",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        2,
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
                                                        "test_grpc_protobuf_export",
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
                                                        "tests/test_grpc_sink.rs",
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
                                                        159,
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
                                                        "test_grpc_sink",
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                ],
                                dropped_attributes_count: 0,
                                flags: 1,
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
                                event_name: "Test log message from within span",
                            },
                            LogRecord {
                                time_unix_nano: 1755687117943732313,
                                observed_time_unix_nano: 1755687117943732313,
                                severity_number: Info,
                                severity_text: "INFO",
                                body: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "Test log message outside span",
                                            ),
                                        ),
                                    },
                                ),
                                attributes: [
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
                                        key: "thread.id",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    IntValue(
                                                        2,
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
                                                        "test_grpc_protobuf_export",
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
                                                        "tests/test_grpc_sink.rs",
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
                                                        161,
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
                                                        "test_grpc_sink",
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                ],
                                dropped_attributes_count: 0,
                                flags: 0,
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
                                    0,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                ],
                                event_name: "Test log message outside span",
                            },
                            LogRecord {
                                time_unix_nano: 1755687117943868666,
                                observed_time_unix_nano: 1755687117943868666,
                                severity_number: Trace,
                                severity_text: "TRACE",
                                body: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "connection accepted",
                                            ),
                                        ),
                                    },
                                ),
                                attributes: [
                                    KeyValue {
                                        key: "logfire.json_schema",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "{}",
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
                                                        2,
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
                                                        "test_grpc_protobuf_export",
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
                                                        "/home/david/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tonic-0.13.1/src/transport/server/mod.rs",
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
                                                        726,
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
                                                        "tonic::transport::server",
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                ],
                                dropped_attributes_count: 0,
                                flags: 0,
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
                                    0,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                ],
                                event_name: "",
                            },
                            LogRecord {
                                time_unix_nano: 1755687117943961578,
                                observed_time_unix_nano: 1755687117943961578,
                                severity_number: Trace,
                                severity_text: "TRACE",
                                body: Some(
                                    AnyValue {
                                        value: Some(
                                            StringValue(
                                                "event /home/david/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/h2-0.4.12/src/proto/connection.rs:273",
                                            ),
                                        ),
                                    },
                                ),
                                attributes: [
                                    KeyValue {
                                        key: "connection.state",
                                        value: Some(
                                            AnyValue {
                                                value: Some(
                                                    StringValue(
                                                        "Open",
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
                                                        "{}",
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
                                                        2,
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
                                                        "test_grpc_protobuf_export",
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
                                                        "/home/david/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/h2-0.4.12/src/proto/connection.rs",
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
                                                        273,
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
                                                        "h2::proto::connection",
                                                    ),
                                                ),
                                            },
                                        ),
                                    },
                                ],
                                dropped_attributes_count: 0,
                                flags: 1,
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
                                    241,
                                ],
                                span_id: [
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                    242,
                                ],
                                event_name: "",
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
