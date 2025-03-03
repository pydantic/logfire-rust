use std::sync::OnceLock;

use log::{LevelFilter, Metadata, Record};
use opentelemetry::{KeyValue, global::ObjectSafeSpan, trace::Tracer};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub(crate) struct LogfireLogger {
    tracer: opentelemetry_sdk::trace::Tracer,
    filter: env_filter::Filter,
}

impl LogfireLogger {
    pub(crate) fn init(tracer: opentelemetry_sdk::trace::Tracer) -> &'static Self {
        static LOGGER: OnceLock<LogfireLogger> = OnceLock::new();
        LOGGER.get_or_init(move || {
            let mut filter_builder = env_filter::Builder::new();
            if let Ok(filter) = std::env::var("RUST_LOG") {
                filter_builder.parse(&filter);
            } else {
                filter_builder.filter_level(log::LevelFilter::Info);
            }

            LogfireLogger {
                tracer,
                filter: filter_builder.build(),
            }
        })
    }

    pub(crate) fn max_level(&self) -> LevelFilter {
        self.filter.filter()
    }
}

impl log::Log for LogfireLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            self.tracer
                .span_builder("log message")
                .with_attributes([
                    KeyValue::new("logfire.msg", record.args().to_string()),
                    KeyValue::new("logfire.level_num", level_to_level_number(record.level())),
                    KeyValue::new("logfire.span_type", "log"),
                ])
                .start_with_context(&self.tracer, &tracing::Span::current().context())
                .end();
        }
    }

    fn flush(&self) {}
}

fn level_to_level_number(level: log::Level) -> i64 {
    match level {
        log::Level::Trace => 1,
        log::Level::Debug => 5,
        log::Level::Info => 9,
        log::Level::Warn => 13,
        log::Level::Error => 17,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use opentelemetry_sdk::trace::SpanData;
    use opentelemetry_sdk::trace::in_memory_exporter::InMemorySpanExporterBuilder;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn test_logfire_logger() {
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let tracer = provider.tracer("test");
        let logger = LogfireLogger::init(tracer.clone());
        tracing::subscriber::with_default(
            tracing_subscriber::registry()
                .with(tracing_opentelemetry::layer().with_tracer(tracer.clone())),
            || {
                let record = Record::builder()
                    .args(format_args!("test message"))
                    .level(log::Level::Info)
                    .target("test")
                    .module_path(Some("test"))
                    .file(Some("test.rs"))
                    .line(Some(42))
                    .build();

                let _ = crate::configure().send_to_logfire(false).finish();
                crate::span!("root span",).in_scope(|| logger.log(&record));
            },
        );

        let mut spans = exporter.get_finished_spans().unwrap();
        assert!(spans.len() == 2);

        let root_span: SpanData = spans.pop().unwrap();
        let root_span_id = root_span.span_context.span_id();
        let trace_id = root_span.span_context.trace_id();

        let mut span: SpanData = spans.pop().unwrap();
        span.attributes.sort_by_key(|k| k.key.clone());
        assert_eq!(span.name, "log message");
        assert_eq!(span.attributes.len(), 3);
        assert_eq!(span.span_context.trace_id(), trace_id);
        assert_eq!(span.parent_span_id, root_span_id);
        assert_eq!(span.attributes[0], KeyValue::new("logfire.level_num", 9i64));
        assert_eq!(
            span.attributes[1],
            KeyValue::new("logfire.msg", "test message")
        );
        assert_eq!(
            span.attributes[2],
            KeyValue::new("logfire.span_type", "log")
        );
    }
}
