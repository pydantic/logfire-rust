//! Integration tests for gRPC sink with mock server.
#![cfg(feature = "export-grpc")]

use insta::assert_debug_snapshot;
use logfire::{configure, set_local_logfire, span};
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

/// Start a mock gRPC server and return its address
async fn start_mock_grpc_server() -> (String, Arc<Mutex<Vec<ExportTraceServiceRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let addr_str = format!("http://{}", addr);

    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    let trace_service = MockTraceService::new(captured_clone);

    tokio::spawn(async move {
        Server::builder()
            .add_service(TraceServiceServer::new(trace_service))
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

    (addr_str, captured)
}

/// Test gRPC protobuf export infrastructure
#[tokio::test]
async fn test_grpc_protobuf_export() {
    use logfire::config::AdvancedOptions;

    let (server_addr, captured) = start_mock_grpc_server().await;

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
    span!("grpc_test_span").in_scope(|| {});
    tokio::task::spawn_blocking(move || guard.shutdown())
        .await
        .unwrap()
        .unwrap();

    let requests = captured.lock().unwrap();

    assert_eq!(requests.len(), 1);

    let mut body = requests[0].clone();
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
                                                        113,
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
}

/// Mutex to avoid concurrent mutation of env vars.
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
