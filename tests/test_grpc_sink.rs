//! Integration tests for gRPC sink with mock server.
// #![cfg(feature = "export-grpc")]

use futures::channel::oneshot;
use insta::assert_debug_snapshot;
use logfire::{configure, info, span};
use opentelemetry::Context;
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

use crate::test_utils::{
    DeterministicIdGenerator, make_log_request_deterministic, make_trace_request_deterministic,
};

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
    oneshot::Sender<()>,
    String,
    Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
    Arc<Mutex<Vec<ExportLogsServiceRequest>>>,
) {
    let (ready_tx, ready_rx) = oneshot::channel();

    // run the server in a separate thread with context suppression
    std::thread::spawn(move || {
        let _guard = Context::enter_telemetry_suppressed_scope();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        rt.block_on(async move {
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

            ready_tx
                .send((shutdown_tx, addr_str, captured_traces, captured_logs))
                .unwrap();

            tokio::select! {
                _ = shutdown_rx => {
                    // Shutdown signal received
                }
            }
        });
    });

    ready_rx.await.unwrap()
}

/// Test gRPC protobuf export infrastructure
#[tokio::test]
async fn test_grpc_protobuf_export() {
    use logfire::config::AdvancedOptions;

    let (shutdown_tx, server_addr, captured_traces, captured_logs) = start_mock_grpc_server().await;

    let env_guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: Holding mutex to prevent other threads interacting with env
    unsafe {
        std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc");
    }

    // NB deliberately not using local logfire here; this way we're validating
    // that any tonic telemetry from the exporter is not captured
    let logfire = configure()
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

    info!("Test log message outside span");
    span!("grpc_test_span").in_scope(|| {
        info!("Test log message from within span");
    });

    tokio::task::spawn_blocking(move || logfire.shutdown())
        .await
        .unwrap()
        .unwrap();

    shutdown_tx.send(()).unwrap();

    let mut trace_requests = std::mem::take(&mut *captured_traces.lock().unwrap());
    let mut log_requests = std::mem::take(&mut *captured_logs.lock().unwrap());

    for req in &mut trace_requests {
        make_trace_request_deterministic(req);
    }

    assert_debug_snapshot!(trace_requests, @r#"
    [
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
                                    name: "grpc_test_span",
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
                                                            189,
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
        },
    ]
    "#);

    for log_request in &mut log_requests {
        make_log_request_deterministic(log_request);
    }

    assert_debug_snapshot!(log_requests, @r#"
    [
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
                                    time_unix_nano: 0,
                                    observed_time_unix_nano: 0,
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
                                                            "test_grpc_sink",
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
                                    time_unix_nano: 1000000000,
                                    observed_time_unix_nano: 1000000000,
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
                                                            190,
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
                            ],
                            schema_url: "",
                        },
                    ],
                    schema_url: "",
                },
            ],
        },
    ]
    "#);
}

/// Mutex to avoid concurrent mutation of env vars.
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
