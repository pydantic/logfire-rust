//! Basic snapshot tests for data produced by the logfire APIs.

use std::sync::Arc;

use insta::assert_debug_snapshot;
use opentelemetry_sdk::{
    Resource,
    metrics::{
        InMemoryMetricExporterBuilder, ManualReader, data::ResourceMetrics,
        exporter::PushMetricExporter, reader::MetricReader,
    },
    trace::{InMemorySpanExporterBuilder, SimpleSpanProcessor},
};
use tracing::{Level, level_filters::LevelFilter};

use logfire::{
    config::{AdvancedOptions, MetricsOptions},
    info, span,
};

#[path = "../src/test_utils.rs"]
mod test_utils;

use test_utils::{DeterministicExporter, DeterministicIdGenerator};

#[expect(clippy::too_many_lines)]
#[test]
fn test_basic_span() {
    let exporter = InMemorySpanExporterBuilder::new().build();

    let handler = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_additional_span_processor(SimpleSpanProcessor::new(DeterministicExporter::new(
            exporter.clone(),
            file!(),
            line!(),
        )))
        .install_panic_handler()
        .with_default_level_filter(LevelFilter::TRACE)
        .with_advanced_options(
            AdvancedOptions::default().with_id_generator(DeterministicIdGenerator::new()),
        )
        .finish()
        .unwrap();

    let guard = logfire::set_local_logfire(handler);

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tracing::subscriber::with_default(guard.subscriber(), || {
            let root = span!("root span").entered();
            let _ = span!("hello world span", attr = "x", dotted.attr = "y").entered();
            let _ = span!(level: Level::DEBUG, "debug span");
            let _ = span!(parent: &root, level: Level::DEBUG, "debug span with explicit parent");
            info!("hello world log", attr = "x", dotted.attr = "y");
            panic!("oh no!");
        });
    }))
    .unwrap_err();

    let spans = exporter.get_finished_spans().unwrap();
    assert_debug_snapshot!(spans, @r#"
    [
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f1,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f0,
            span_kind: Internal,
            name: "root span",
            start_time: SystemTime {
                tv_sec: 0,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 0,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        14,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "root span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Owned(
                            "{\"type\":\"object\",\"properties\":{}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        9,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "pending_span",
                        ),
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f3,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f2,
            span_kind: Internal,
            name: "hello world span",
            start_time: SystemTime {
                tv_sec: 1,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 1,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        15,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "attr",
                    ),
                    value: String(
                        Owned(
                            "x",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "dotted.attr",
                    ),
                    value: String(
                        Owned(
                            "y",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "hello world span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Owned(
                            "{\"type\":\"object\",\"properties\":{\"attr\":{},\"dotted.attr\":{}}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        9,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "pending_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.pending_parent_id",
                    ),
                    value: String(
                        Owned(
                            "00000000000000f0",
                        ),
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f2,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f0,
            span_kind: Internal,
            name: "hello world span",
            start_time: SystemTime {
                tv_sec: 1,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 2,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        15,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "attr",
                    ),
                    value: String(
                        Owned(
                            "x",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "dotted.attr",
                    ),
                    value: String(
                        Owned(
                            "y",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "hello world span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Owned(
                            "{\"type\":\"object\",\"properties\":{\"attr\":{},\"dotted.attr\":{}}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        9,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "busy_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "idle_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f4,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f0,
            span_kind: Internal,
            name: "debug span",
            start_time: SystemTime {
                tv_sec: 3,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 4,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        16,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "debug span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Owned(
                            "{\"type\":\"object\",\"properties\":{}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        5,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "busy_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "idle_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f5,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f0,
            span_kind: Internal,
            name: "debug span with explicit parent",
            start_time: SystemTime {
                tv_sec: 5,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 6,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        17,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "debug span with explicit parent",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Owned(
                            "{\"type\":\"object\",\"properties\":{}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        5,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "busy_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "idle_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f6,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f0,
            span_kind: Internal,
            name: "hello world log",
            start_time: SystemTime {
                tv_sec: 7,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 7,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "attr",
                    ),
                    value: String(
                        Static(
                            "x",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "dotted.attr",
                    ),
                    value: String(
                        Static(
                            "y",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "hello world log",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        9,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "log",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Static(
                            "{\"type\":\"object\",\"properties\":{\"attr\":{},\"dotted.attr\":{}}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        18,
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f7,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f0,
            span_kind: Internal,
            name: "panic: {message}",
            start_time: SystemTime {
                tv_sec: 8,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 8,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "location",
                    ),
                    value: String(
                        Owned(
                            "tests/test_basic_exports.rs:56:13",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "backtrace",
                    ),
                    value: String(
                        Owned(
                            "disabled backtrace",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "panic: oh no!",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        17,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "log",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Static(
                            "{\"type\":\"object\",\"properties\":{\"location\":{},\"backtrace\":{}}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "src/lib.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        698,
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "logfire",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
        SpanData {
            span_context: SpanContext {
                trace_id: 000000000000000000000000000000f0,
                span_id: 00000000000000f0,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 0000000000000000,
            span_kind: Internal,
            name: "root span",
            start_time: SystemTime {
                tv_sec: 0,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 9,
                tv_nsec: 0,
            },
            attributes: [
                KeyValue {
                    key: Static(
                        "code.filepath",
                    ),
                    value: String(
                        Static(
                            "tests/test_basic_exports.rs",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.namespace",
                    ),
                    value: String(
                        Static(
                            "test_basic_exports",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "code.lineno",
                    ),
                    value: I64(
                        14,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.id",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "thread.name",
                    ),
                    value: String(
                        Owned(
                            "test_basic_span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.msg",
                    ),
                    value: String(
                        Owned(
                            "root span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.json_schema",
                    ),
                    value: String(
                        Owned(
                            "{\"type\":\"object\",\"properties\":{}}",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.level_num",
                    ),
                    value: I64(
                        9,
                    ),
                },
                KeyValue {
                    key: Static(
                        "logfire.span_type",
                    ),
                    value: String(
                        Static(
                            "span",
                        ),
                    ),
                },
                KeyValue {
                    key: Static(
                        "busy_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
                KeyValue {
                    key: Static(
                        "idle_ns",
                    ),
                    value: I64(
                        0,
                    ),
                },
            ],
            dropped_attributes_count: 0,
            events: SpanEvents {
                events: [],
                dropped_count: 0,
            },
            links: SpanLinks {
                links: [],
                dropped_count: 0,
            },
            status: Unset,
            instrumentation_scope: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
        },
    ]
    "#);
}

#[derive(Clone, Debug)]
struct SharedManualReader {
    reader: Arc<ManualReader>,
}

impl SharedManualReader {
    fn new(reader: ManualReader) -> Self {
        Self {
            reader: Arc::new(reader),
        }
    }

    async fn export<E: PushMetricExporter>(&self, exporter: &E) {
        let mut metrics = ResourceMetrics {
            resource: Resource::builder_empty().build(),
            scope_metrics: Vec::new(),
        };
        dbg!(&metrics);
        self.reader.collect(&mut metrics).unwrap();
        dbg!(&metrics);
        exporter.export(&mut metrics).await.unwrap();
    }
}

impl MetricReader for SharedManualReader {
    fn register_pipeline(&self, pipeline: std::sync::Weak<opentelemetry_sdk::metrics::Pipeline>) {
        self.reader.register_pipeline(pipeline);
    }

    fn collect(
        &self,
        rm: &mut opentelemetry_sdk::metrics::data::ResourceMetrics,
    ) -> opentelemetry_sdk::metrics::MetricResult<()> {
        self.reader.collect(rm)
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.reader.force_flush()
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.reader.shutdown()
    }

    fn temporality(
        &self,
        kind: opentelemetry_sdk::metrics::InstrumentKind,
    ) -> opentelemetry_sdk::metrics::Temporality {
        self.reader.temporality(kind)
    }
}

#[tokio::test]
async fn test_basic_metrics() {
    let mut exporter = DeterministicExporter::new(
        InMemoryMetricExporterBuilder::new().build(),
        file!(),
        line!(),
    );

    let reader = SharedManualReader::new(
        ManualReader::builder()
            .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
            .build(),
    );

    let handler = logfire::configure()
        .send_to_logfire(false)
        .with_metrics(Some(
            MetricsOptions::default().with_additional_reader(reader.clone()),
        ))
        .with_advanced_options(
            AdvancedOptions::default()
                .with_resource(Resource::builder_empty().with_service_name("test").build()),
        )
        .finish()
        .unwrap();

    let guard = logfire::set_local_logfire(handler.clone());

    use opentelemetry::metrics::MeterProvider;

    let counter = guard
        .meter_provider()
        .meter("logfire")
        .u64_counter("basic_counter")
        .build();

    counter.add(1, &[]);
    reader.export(&mut exporter).await;

    counter.add(2, &[]);
    reader.export(&mut exporter).await;

    handler.shutdown().unwrap();

    let metrics = exporter.inner().get_finished_metrics().unwrap();

    assert_debug_snapshot!(metrics, @r#"
    [
        ResourceMetrics {
            resource: Resource {
                inner: ResourceInner {
                    attrs: {
                        Static(
                            "service.name",
                        ): String(
                            Static(
                                "test",
                            ),
                        ),
                    },
                    schema_url: None,
                },
            },
            scope_metrics: [
                ScopeMetrics {
                    scope: InstrumentationScope {
                        name: "logfire",
                        version: None,
                        schema_url: None,
                        attributes: [],
                    },
                    metrics: [
                        Metric {
                            name: "basic_counter",
                            description: "",
                            unit: "",
                            data: Sum {
                                data_points: [
                                    SumDataPoint {
                                        attributes: [],
                                        value: 1,
                                        exemplars: [],
                                    },
                                ],
                                start_time: SystemTime {
                                    tv_sec: 0,
                                    tv_nsec: 0,
                                },
                                time: SystemTime {
                                    tv_sec: 1,
                                    tv_nsec: 0,
                                },
                                temporality: Delta,
                                is_monotonic: true,
                            },
                        },
                    ],
                },
            ],
        },
        ResourceMetrics {
            resource: Resource {
                inner: ResourceInner {
                    attrs: {
                        Static(
                            "service.name",
                        ): String(
                            Static(
                                "test",
                            ),
                        ),
                    },
                    schema_url: None,
                },
            },
            scope_metrics: [
                ScopeMetrics {
                    scope: InstrumentationScope {
                        name: "logfire",
                        version: None,
                        schema_url: None,
                        attributes: [],
                    },
                    metrics: [
                        Metric {
                            name: "basic_counter",
                            description: "",
                            unit: "",
                            data: Sum {
                                data_points: [
                                    SumDataPoint {
                                        attributes: [],
                                        value: 2,
                                        exemplars: [],
                                    },
                                ],
                                start_time: SystemTime {
                                    tv_sec: 1,
                                    tv_nsec: 0,
                                },
                                time: SystemTime {
                                    tv_sec: 2,
                                    tv_nsec: 0,
                                },
                                temporality: Delta,
                                is_monotonic: true,
                            },
                        },
                    ],
                },
            ],
        },
    ]
    "#)
}
