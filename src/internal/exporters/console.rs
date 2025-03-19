use std::{
    borrow::Cow,
    io::{self, BufWriter, Write},
    sync::{Arc, LazyLock},
};

use chrono::{DateTime, Utc};
use futures_util::future::BoxFuture;
use nu_ansi_term::{Color, Style};
use opentelemetry::Value;
use opentelemetry_sdk::trace::SpanData;
use tracing::field::Visit;

use crate::{
    bridges::tracing::level_to_level_number,
    config::{ConsoleOptions, Target},
    internal::constants::ATTRIBUTES_SPAN_TYPE_KEY,
};

/// Simple span exporter which attempts to match the "simple" Python logfire console exporter.
#[derive(Debug)]
pub struct SimpleConsoleSpanExporter {
    stopped: bool,
    writer: Arc<ConsoleWriter>,
}

impl SimpleConsoleSpanExporter {
    pub fn new(writer: Arc<ConsoleWriter>) -> Self {
        Self {
            writer,
            stopped: false,
        }
    }
}

impl opentelemetry_sdk::trace::SpanExporter for SimpleConsoleSpanExporter {
    fn export(
        &mut self,
        batch: Vec<opentelemetry_sdk::trace::SpanData>,
    ) -> BoxFuture<'static, opentelemetry_sdk::error::OTelSdkResult> {
        if !self.stopped {
            self.writer.write_batch(&batch);
        }
        Box::pin(std::future::ready(Ok(())))
    }

    fn shutdown(&mut self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.stopped = true;
        Ok(())
    }
}

static DIMMED: LazyLock<Style> = LazyLock::new(|| Style::new().dimmed());
static DIMMED_AND_ITALIC: LazyLock<Style> = LazyLock::new(|| DIMMED.italic());
static BOLD: LazyLock<Style> = LazyLock::new(|| Style::new().bold());
static ITALIC: LazyLock<Style> = LazyLock::new(|| Style::new().italic());

fn level_int_to_text<W: io::Write>(level: i64, w: &mut W) -> io::Result<()> {
    match level {
        1 => write!(w, "{}", Color::Purple.paint(" TRACE")),
        2..=5 => write!(w, "{}", Color::Blue.paint(" DEBUG")),
        6..=9 => write!(w, "{}", Color::Green.paint("  INFO")),
        10..=13 => write!(w, "{}", Color::Yellow.paint("  WARN")),
        14.. => write!(w, "{}", Color::Red.paint(" ERROR")),
        _ => write!(w, "{}", Color::DarkGray.paint(" -----")),
    }
}

#[derive(Debug)]
pub struct ConsoleWriter {
    options: ConsoleOptions,
}

impl ConsoleWriter {
    pub fn new(options: ConsoleOptions) -> Self {
        Self { options }
    }

    pub fn write_batch(&self, batch: &[opentelemetry_sdk::trace::SpanData]) {
        self.with_writer(|w| {
            let mut buffer = BufWriter::new(w);
            for span in batch {
                let _ = Self::span_to_writer(span, &mut buffer);
            }
        });
    }

    pub fn write_tracing_event(&self, event: &tracing::Event<'_>) {
        self.with_writer(|w| {
            let mut buffer = BufWriter::new(w);
            let _ = Self::event_to_writer(event, &mut buffer);
        });
    }

    fn with_writer<R>(&self, f: impl FnOnce(&mut dyn Write) -> R) -> R {
        match &self.options.target {
            Target::Stdout => f(&mut io::stdout()),
            Target::Stderr => f(&mut io::stderr()),
            Target::Pipe(p) => f(&mut *p.lock().expect("pipe lock poisoned")),
        }
    }

    fn span_to_writer<W: io::Write>(span: &SpanData, w: &mut W) -> io::Result<()> {
        let span_type = span
            .attributes
            .iter()
            .find_map(|attr| {
                if attr.key.as_str() == ATTRIBUTES_SPAN_TYPE_KEY {
                    Some(attr.value.as_str())
                } else {
                    None
                }
            })
            .unwrap_or(Cow::Borrowed("span"));

        // only print for pending span and logs
        if span_type == "span" {
            return Ok(());
        }

        let timestamp: DateTime<Utc> = span.start_time.into();
        let mut msg = None;
        let mut level = None;
        let mut target = None;

        let mut fields = Vec::new();

        for kv in &span.attributes {
            match kv.key.as_str() {
                "logfire.msg" => {
                    msg = Some(kv.value.as_str());
                }
                "logfire.level_num" => {
                    if let Value::I64(val) = kv.value {
                        level = Some(val);
                    }
                }
                "code.namespace" => target = Some(kv.value.as_str()),
                // Filter out known values
                ATTRIBUTES_SPAN_TYPE_KEY
                | "logfire.json_schema"
                | "logfire.pending_parent_id"
                | "code.filepath"
                | "code.lineno"
                | "thread.id"
                | "thread.name"
                | "logfire.null_args"
                | "busy_ns"
                | "idle_ns" => (),
                _ => {
                    fields.push(kv);
                }
            }
        }

        if msg.is_none() {
            msg = Some(span.name.clone());
        }

        write!(
            w,
            "{}",
            DIMMED.paint(timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
        )?;
        if let Some(level) = level {
            level_int_to_text(level, w)?;
        }

        if let Some(target) = target {
            write!(w, " {}", DIMMED_AND_ITALIC.paint(target))?;
        }

        if let Some(msg) = msg {
            write!(w, " {}", BOLD.paint(msg))?;
        }

        if !fields.is_empty() {
            for (idx, kv) in fields.iter().enumerate() {
                let key = kv.key.as_str();
                let value = kv.value.as_str();
                write!(w, " {}={value}", ITALIC.paint(key))?;
                if idx < fields.len() - 1 {
                    write!(w, ",")?;
                }
            }
        }

        writeln!(w)
    }

    fn event_to_writer<W: io::Write>(event: &tracing::Event<'_>, w: &mut W) -> io::Result<()> {
        let timestamp: DateTime<Utc> = Utc::now();
        let level = level_to_level_number(*event.metadata().level());
        let target = event.metadata().module_path();

        let mut visitor = FieldsVisitor {
            message: None,
            // TODO: support formatting the fields? Maybe according to `ConsoleOptions`.
            // fields: Vec::new(),
        };

        event.record(&mut visitor);

        let msg = visitor
            .message
            .unwrap_or_else(|| event.metadata().name().to_string());

        write!(
            w,
            "{}",
            DIMMED.paint(timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
        )?;

        level_int_to_text(level, w)?;

        if let Some(target) = target {
            write!(w, " {}", DIMMED_AND_ITALIC.paint(target))?;
        }

        write!(w, " {}", BOLD.paint(msg))?;

        writeln!(w)
    }
}

/// Internal helper to `visit` a `tracing::Event` and collect relevant fields.
struct FieldsVisitor {
    message: Option<String>,
    // fields: Vec<(&'static str, String)>,
}

impl Visit for FieldsVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            // self.fields.push((field.name(), value.to_string()));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            // self.fields.push((field.name(), format!("{value:?}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use insta::assert_snapshot;
    use opentelemetry_sdk::trace::SimpleSpanProcessor;
    use tracing::{Level, level_filters::LevelFilter};

    use crate::{
        config::{ConsoleOptions, Target},
        internal::exporters::console::{ConsoleWriter, SimpleConsoleSpanExporter},
        set_local_logfire,
        tests::DeterministicExporter,
    };

    #[test]
    fn test_print_to_console() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default()
        };

        let config = crate::configure()
            .send_to_logfire(false)
            .with_additional_span_processor(SimpleSpanProcessor::new(Box::new(
                DeterministicExporter::new(
                    SimpleConsoleSpanExporter::new(ConsoleWriter::new(console_options).into()),
                    file!(),
                    line!(),
                ),
            )))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE);

        let guard = set_local_logfire(config).unwrap();

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tracing::subscriber::with_default(guard.subscriber.clone(), || {
                let root = crate::span!("root span").entered();
                let _ = crate::span!("hello world span").entered();
                let _ = crate::span!(level: Level::DEBUG, "debug span");
                let _ = crate::span!(parent: &root, level: Level::DEBUG, "debug span with explicit parent");
                crate::info!("hello world log");
                panic!("oh no!");
            });
        }))
        .unwrap_err();

        guard.shutdown_handler.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert_snapshot!(output, @r#"
        [2m1970-01-01T00:00:00.000000Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mroot span[0m
        [2m1970-01-01T00:00:01.000000Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world span[0m
        [2m1970-01-01T00:00:03.000000Z[0m[34m DEBUG[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mdebug span[0m
        [2m1970-01-01T00:00:05.000000Z[0m[34m DEBUG[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mdebug span with explicit parent[0m
        [2m1970-01-01T00:00:07.000000Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world log[0m
        [2m1970-01-01T00:00:08.000000Z[0m[31m ERROR[0m [2;3mlogfire[0m [1mpanic: oh no![0m [3mlocation[0m=src/internal/exporters/console.rs:295:17, [3mbacktrace[0m=disabled backtrace
        "#);
    }
}
