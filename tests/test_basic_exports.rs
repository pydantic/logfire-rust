//! Basic snapshot tests for data produced by the logfire APIs.

use std::sync::Arc;

use insta::assert_debug_snapshot;
use opentelemetry_sdk::{
    Resource,
    logs::{InMemoryLogExporter, SimpleLogProcessor},
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

use test_utils::{
    DeterministicExporter, DeterministicIdGenerator, make_deterministic_logs,
    make_deterministic_resource_metrics,
};

#[expect(clippy::too_many_lines)]
#[test]
fn test_basic_span() {
    const TEST_FILE: &str = file!();
    const TEST_LINE: u32 = line!();

    let exporter = InMemorySpanExporterBuilder::new().build();
    let log_exporter = InMemoryLogExporter::default();

    let handler = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_additional_span_processor(SimpleSpanProcessor::new(DeterministicExporter::new(
            exporter.clone(),
            TEST_FILE,
            TEST_LINE,
        )))
        .install_panic_handler()
        .with_default_level_filter(LevelFilter::TRACE)
        .with_advanced_options(
            AdvancedOptions::default()
                .with_id_generator(DeterministicIdGenerator::new())
                .with_log_processor(SimpleLogProcessor::new(log_exporter.clone())),
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
                        27,
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
                        28,
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
                        28,
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
                span_id: 00000000000000f5,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f4,
            span_kind: Internal,
            name: "debug span",
            start_time: SystemTime {
                tv_sec: 3,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 3,
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
                        29,
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
                        29,
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
                span_id: 00000000000000f7,
                trace_flags: TraceFlags(
                    1,
                ),
                is_remote: false,
                trace_state: TraceState(
                    None,
                ),
            },
            parent_span_id: 00000000000000f6,
            span_kind: Internal,
            name: "debug span with explicit parent",
            start_time: SystemTime {
                tv_sec: 5,
                tv_nsec: 0,
            },
            end_time: SystemTime {
                tv_sec: 5,
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
                        30,
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
                        30,
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
                tv_sec: 7,
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
                        27,
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

    let logs = log_exporter.get_emitted_logs().unwrap();
    let logs = make_deterministic_logs(logs, TEST_FILE, TEST_LINE);
    assert_debug_snapshot!(logs, @r#"
    [
        LogDataWithResource {
            record: SdkLogRecord {
                event_name: None,
                target: None,
                timestamp: Some(
                    SystemTime {
                        tv_sec: 0,
                        tv_nsec: 0,
                    },
                ),
                observed_timestamp: Some(
                    SystemTime {
                        tv_sec: 0,
                        tv_nsec: 0,
                    },
                ),
                trace_context: Some(
                    TraceContext {
                        trace_id: 000000000000000000000000000000f0,
                        span_id: 00000000000000f0,
                        trace_flags: Some(
                            TraceFlags(
                                1,
                            ),
                        ),
                    },
                ),
                severity_text: Some(
                    "INFO",
                ),
                severity_number: Some(
                    Info,
                ),
                body: Some(
                    String(
                        Owned(
                            "hello world log",
                        ),
                    ),
                ),
                attributes: GrowableArray {
                    inline: [
                        Some(
                            (
                                Static(
                                    "attr",
                                ),
                                String(
                                    Static(
                                        "x",
                                    ),
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "code.filepath",
                                ),
                                String(
                                    Static(
                                        "tests/test_basic_exports.rs",
                                    ),
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "code.lineno",
                                ),
                                Int(
                                    31,
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "code.namespace",
                                ),
                                String(
                                    Static(
                                        "test_basic_exports",
                                    ),
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "dotted.attr",
                                ),
                                String(
                                    Static(
                                        "y",
                                    ),
                                ),
                            ),
                        ),
                    ],
                    overflow: Some(
                        [
                            Some(
                                (
                                    Static(
                                        "logfire.json_schema",
                                    ),
                                    String(
                                        Static(
                                            "{\"type\":\"object\",\"properties\":{\"attr\":{},\"dotted.attr\":{}}}",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.level_num",
                                    ),
                                    Int(
                                        9,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.msg",
                                    ),
                                    String(
                                        Owned(
                                            "hello world log",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "thread.id",
                                    ),
                                    Int(
                                        0,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "thread.name",
                                    ),
                                    String(
                                        Owned(
                                            "test_basic_span",
                                        ),
                                    ),
                                ),
                            ),
                        ],
                    ),
                    count: 5,
                },
            },
            instrumentation: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
            resource: Resource {
                inner: ResourceInner {
                    attrs: {},
                    schema_url: None,
                },
            },
        },
        LogDataWithResource {
            record: SdkLogRecord {
                event_name: None,
                target: None,
                timestamp: Some(
                    SystemTime {
                        tv_sec: 1,
                        tv_nsec: 0,
                    },
                ),
                observed_timestamp: Some(
                    SystemTime {
                        tv_sec: 1,
                        tv_nsec: 0,
                    },
                ),
                trace_context: Some(
                    TraceContext {
                        trace_id: 000000000000000000000000000000f0,
                        span_id: 00000000000000f0,
                        trace_flags: Some(
                            TraceFlags(
                                1,
                            ),
                        ),
                    },
                ),
                severity_text: Some(
                    "ERROR",
                ),
                severity_number: Some(
                    Error,
                ),
                body: Some(
                    String(
                        Owned(
                            "panic: oh no!",
                        ),
                    ),
                ),
                attributes: GrowableArray {
                    inline: [
                        Some(
                            (
                                Static(
                                    "backtrace",
                                ),
                                String(
                                    Owned(
                                        "disabled backtrace",
                                    ),
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "code.filepath",
                                ),
                                String(
                                    Owned(
                                        "tests/test_basic_exports.rs",
                                    ),
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "code.lineno",
                                ),
                                Int(
                                    32,
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "logfire.json_schema",
                                ),
                                String(
                                    Static(
                                        "{\"type\":\"object\",\"properties\":{\"backtrace\":{}}}",
                                    ),
                                ),
                            ),
                        ),
                        Some(
                            (
                                Static(
                                    "logfire.level_num",
                                ),
                                Int(
                                    17,
                                ),
                            ),
                        ),
                    ],
                    overflow: Some(
                        [
                            Some(
                                (
                                    Static(
                                        "logfire.msg",
                                    ),
                                    String(
                                        Owned(
                                            "panic: oh no!",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "thread.id",
                                    ),
                                    Int(
                                        0,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "thread.name",
                                    ),
                                    String(
                                        Owned(
                                            "test_basic_span",
                                        ),
                                    ),
                                ),
                            ),
                        ],
                    ),
                    count: 5,
                },
            },
            instrumentation: InstrumentationScope {
                name: "logfire",
                version: None,
                schema_url: None,
                attributes: [],
            },
            resource: Resource {
                inner: ResourceInner {
                    attrs: {},
                    schema_url: None,
                },
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
        let mut metrics = ResourceMetrics::default();
        self.reader.collect(&mut metrics).unwrap();
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
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.reader.collect(rm)
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.reader.force_flush()
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.reader.shutdown()
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.reader.shutdown_with_timeout(timeout)
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
    let mut exporter = InMemoryMetricExporterBuilder::new().build();

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

    let metrics = exporter.get_finished_metrics().unwrap();

    let metrics = make_deterministic_resource_metrics(metrics);

    assert_debug_snapshot!(metrics, @r#"
    [
        DeterministicResourceMetrics {
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
                DeterministicScopeMetrics {
                    scope: InstrumentationScope {
                        name: "logfire",
                        version: None,
                        schema_url: None,
                        attributes: [],
                    },
                    sum_metrics: [
                        [
                            1,
                        ],
                    ],
                },
            ],
        },
        DeterministicResourceMetrics {
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
                DeterministicScopeMetrics {
                    scope: InstrumentationScope {
                        name: "logfire",
                        version: None,
                        schema_url: None,
                        attributes: [],
                    },
                    sum_metrics: [
                        [
                            2,
                        ],
                    ],
                },
            ],
        },
    ]
    "#)
}
