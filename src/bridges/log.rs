use std::{borrow::Cow, sync::OnceLock};

use log::{LevelFilter, Metadata, Record};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::internal::logfire_tracer::LogfireTracer;

pub(crate) struct LogfireLogger {
    tracer: LogfireTracer,
}

impl LogfireLogger {
    pub(crate) fn init(tracer: LogfireTracer) -> &'static Self {
        static LOGGER: OnceLock<LogfireLogger> = OnceLock::new();
        LOGGER.get_or_init(move || LogfireLogger { tracer })
    }

    pub(crate) fn max_level(&self) -> LevelFilter {
        self.tracer.filter.filter()
    }
}

impl log::Log for LogfireLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.tracer.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            self.tracer.export_log(
                "log message",
                &tracing::Span::current().context(),
                record.args().to_string(),
                level_to_severity(record.level()),
                "{}",
                record.file().map(|s| Cow::Owned(s.to_string())),
                record.line(),
                record.module_path().map(|s| Cow::Owned(s.to_string())),
                [],
            );
        }
    }

    fn flush(&self) {}
}

fn level_to_severity(level: log::Level) -> opentelemetry::logs::Severity {
    match level {
        log::Level::Error => opentelemetry::logs::Severity::Error,
        log::Level::Warn => opentelemetry::logs::Severity::Warn,
        log::Level::Info => opentelemetry::logs::Severity::Info,
        log::Level::Debug => opentelemetry::logs::Severity::Debug,
        log::Level::Trace => opentelemetry::logs::Severity::Trace,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use insta::{assert_debug_snapshot, assert_snapshot};
    use opentelemetry_sdk::logs::in_memory_exporter::InMemoryLogExporter;
    use tracing::level_filters::LevelFilter as TracingLevelFilter;

    use crate::{
        bridges::log::LogfireLogger,
        config::{AdvancedOptions, ConsoleOptions, Target},
        set_local_logfire,
        test_utils::{
            DeterministicIdGenerator, make_deterministic_logs, remap_timestamps_in_console_output,
        },
    };

    #[test]
    fn test_log_bridge() {
        const TEST_FILE: &str = file!();
        const TEST_LINE: u32 = line!();

        let log_exporter = InMemoryLogExporter::default();

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .install_panic_handler()
            .with_default_level_filter(TracingLevelFilter::TRACE)
            .with_advanced_options(
                AdvancedOptions::default()
                    .with_id_generator(DeterministicIdGenerator::new())
                    .with_log_processor(opentelemetry_sdk::logs::SimpleLogProcessor::new(
                        log_exporter.clone(),
                    )),
            )
            .finish()
            .unwrap();

        let lf_logger = LogfireLogger {
            tracer: handler.tracer.clone(),
        };

        log::set_max_level(log::LevelFilter::Trace);
        let guard = set_local_logfire(handler);

        tracing::subscriber::with_default(guard.subscriber().clone(), || {
            log::info!(logger: lf_logger, "root event");
            log::info!(logger: lf_logger, target: "custom_target", "root event with target");

            let _root = tracing::span!(tracing::Level::INFO, "root span").entered();
            log::info!(logger: lf_logger, "hello world log");
            log::warn!(logger: lf_logger, "warning log");
            log::error!(logger: lf_logger, "error log");
            log::debug!(logger: lf_logger, "debug log");
            log::trace!(logger: lf_logger, "trace log");
        });

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
                            trace_id: 00000000000000000000000000000000,
                            span_id: 0000000000000000,
                            trace_flags: Some(
                                TraceFlags(
                                    0,
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
                                "root event",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        27,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                                            "root event",
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
                                                "bridges::log::tests::test_log_bridge",
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
                            trace_id: 00000000000000000000000000000000,
                            span_id: 0000000000000000,
                            trace_flags: Some(
                                TraceFlags(
                                    0,
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
                                "root event with target",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        28,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                                            "root event with target",
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
                                                "bridges::log::tests::test_log_bridge",
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
                            tv_sec: 2,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 2,
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
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                        ],
                        overflow: Some(
                            [
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
                                                "bridges::log::tests::test_log_bridge",
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
                            tv_sec: 3,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 3,
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
                        "WARN",
                    ),
                    severity_number: Some(
                        Warn,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "warning log",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        "code.namespace",
                                    ),
                                    String(
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                                            "warning log",
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
                                                "bridges::log::tests::test_log_bridge",
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
                            tv_sec: 4,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 4,
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
                                "error log",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        33,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                                            "error log",
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
                                                "bridges::log::tests::test_log_bridge",
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
                            tv_sec: 5,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 5,
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
                        "DEBUG",
                    ),
                    severity_number: Some(
                        Debug,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "debug log",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        34,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                                            "debug log",
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
                                                "bridges::log::tests::test_log_bridge",
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
                            tv_sec: 6,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 6,
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
                        "TRACE",
                    ),
                    severity_number: Some(
                        Trace,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "trace log",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Owned(
                                            "src/bridges/log.rs",
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
                                        35,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Owned(
                                            "logfire::bridges::log::tests",
                                        ),
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
                                            "{}",
                                        ),
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
                                            "trace log",
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
                                                "bridges::log::tests::test_log_bridge",
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

    #[test]
    fn test_log_bridge_console_output() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(tracing::Level::TRACE)
        };

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options.clone()))
            .install_panic_handler()
            .with_default_level_filter(TracingLevelFilter::TRACE)
            .finish()
            .unwrap();

        let lf_logger = LogfireLogger {
            tracer: handler.tracer.clone(),
        };

        log::set_max_level(log::LevelFilter::Trace);
        let guard = crate::set_local_logfire(handler);

        tracing::subscriber::with_default(guard.subscriber().clone(), || {
            log::info!(logger: lf_logger, "root event");
            log::info!(logger: lf_logger, target: "custom_target", "root event with target");

            let _root = tracing::span!(tracing::Level::INFO, "root span").entered();
            log::info!(logger: lf_logger, "hello world log");
            log::warn!(logger: lf_logger, "warning log");
            log::error!(logger: lf_logger, "error log");
            log::trace!(logger: lf_logger, "trace log");
        });

        guard.shutdown_handler.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();
        let output = remap_timestamps_in_console_output(output);

        assert_snapshot!(output, @r"
        [2m1970-01-01T00:00:00.000000Z[0m[32m  INFO[0m [2;3mlogfire::bridges::log::tests[0m [1mroot event[0m
        [2m1970-01-01T00:00:00.000001Z[0m[32m  INFO[0m [2;3mlogfire::bridges::log::tests[0m [1mroot event with target[0m
        [2m1970-01-01T00:00:00.000002Z[0m[32m  INFO[0m [2;3mlogfire::bridges::log::tests[0m [1mroot span[0m
        [2m1970-01-01T00:00:00.000003Z[0m[32m  INFO[0m [2;3mlogfire::bridges::log::tests[0m [1mhello world log[0m
        [2m1970-01-01T00:00:00.000004Z[0m[33m  WARN[0m [2;3mlogfire::bridges::log::tests[0m [1mwarning log[0m
        [2m1970-01-01T00:00:00.000005Z[0m[31m ERROR[0m [2;3mlogfire::bridges::log::tests[0m [1merror log[0m
        [2m1970-01-01T00:00:00.000006Z[0m[35m TRACE[0m [2;3mlogfire::bridges::log::tests[0m [1mtrace log[0m
        [2m1970-01-01T00:00:00.000007Z[0m[34m DEBUG[0m [2;3mopentelemetry_sdk::metrics::meter_provider[0m [1mUser initiated shutdown of MeterProvider.[0m [3mname[0m=MeterProvider.Shutdown
        [2m1970-01-01T00:00:00.000008Z[0m[34m DEBUG[0m [2;3mopentelemetry_sdk::logs::logger_provider[0m [1m[0m [3mname[0m=LoggerProvider.ShutdownInvokedByUser
        ");
    }
}
