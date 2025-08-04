use std::{
    io::{self, BufWriter, Write},
    sync::{Arc, LazyLock, Mutex, mpsc},
};

use chrono::{DateTime, Utc};
use nu_ansi_term::{Color, Style};
use opentelemetry::{Value, logs::AnyValue};
use opentelemetry_sdk::logs::SdkLogRecord;
use opentelemetry_sdk::trace::SpanData;

use crate::{
    bridges::tracing::tracing_level_to_severity,
    config::{ConsoleOptions, Target},
    internal::{constants::ATTRIBUTES_SPAN_TYPE_KEY, span_data_ext::SpanDataExt},
};

/// Enum to represent different types of telemetry data that can be written to console
#[derive(Debug)]
enum ConsoleItem {
    Span(SpanData),
    Log(SdkLogRecord),
}

/// Shared state for console processors
#[derive(Debug)]
struct ConsoleSharedState {
    tx: mpsc::Sender<ConsoleItem>,
    thread_handle: std::thread::JoinHandle<()>,
}

impl ConsoleSharedState {
    fn shutdown_with_timeout(
        self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        // Close the channel to signal shutdown
        drop(self.tx);
        // Wait for the thread to finish, polling every 10ms
        let start = std::time::Instant::now();
        loop {
            if self.thread_handle.is_finished() {
                self.thread_handle.join().expect("failed to join thread");
                break;
            }
            if start.elapsed() >= timeout {
                return Err(opentelemetry_sdk::error::OTelSdkError::Timeout(timeout));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }
}

/// Create both console processors that share a single background thread
pub fn create_console_processors(
    writer: Arc<ConsoleWriter>,
) -> (SimpleConsoleSpanProcessor, SimpleConsoleLogProcessor) {
    let (tx, rx) = mpsc::channel();

    // Spawn a single thread to handle both spans and logs
    let thread_handle = std::thread::spawn(move || {
        while let Ok(item) = rx.recv() {
            match item {
                ConsoleItem::Span(span) => writer.write_batch(&[span]),
                ConsoleItem::Log(log_record) => writer.write_log_batch(&[log_record]),
            }
        }
    });

    let shared_state = Arc::new(ConsoleSharedState { tx, thread_handle });

    let span_processor = SimpleConsoleSpanProcessor {
        shared_state: Mutex::new(Some(shared_state.clone())),
    };

    let log_processor = SimpleConsoleLogProcessor {
        shared_state: Mutex::new(Some(shared_state)),
    };

    (span_processor, log_processor)
}

/// Simple span processor which sends spans to the shared console writer.
#[derive(Debug)]
pub struct SimpleConsoleSpanProcessor {
    shared_state: Mutex<Option<Arc<ConsoleSharedState>>>,
}

impl opentelemetry_sdk::trace::SpanProcessor for SimpleConsoleSpanProcessor {
    fn on_start(&self, _span: &mut opentelemetry_sdk::trace::Span, _cx: &opentelemetry::Context) {}

    fn on_end(&self, span: opentelemetry_sdk::trace::SpanData) {
        if let Some(state) = self.shared_state.lock().expect("no poisoning").as_mut() {
            state
                .tx
                .send(ConsoleItem::Span(span))
                .expect("failed to send span to console writer");
        }
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        if let Some(state) = self
            .shared_state
            .lock()
            .expect("no poisoning")
            .take()
            .and_then(Arc::into_inner)
        {
            state.shutdown_with_timeout(timeout)?;
        }
        Ok(())
    }
}

/// Simple log processor which sends logs to the shared console writer.
#[derive(Debug)]
pub struct SimpleConsoleLogProcessor {
    shared_state: Mutex<Option<Arc<ConsoleSharedState>>>,
}

impl opentelemetry_sdk::logs::LogProcessor for SimpleConsoleLogProcessor {
    fn emit(
        &self,
        log_record: &mut SdkLogRecord,
        _instrumentation_scope: &opentelemetry::InstrumentationScope,
    ) {
        if let Some(state) = self.shared_state.lock().expect("no poisoning").as_mut() {
            state
                .tx
                .send(ConsoleItem::Log(log_record.clone()))
                .expect("failed to send log to console writer");
        }
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        if let Some(state) = self
            .shared_state
            .lock()
            .expect("no poisoning")
            .take()
            .and_then(Arc::into_inner)
        {
            state.shutdown_with_timeout(timeout)?;
        }
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
                let _ = self.span_to_writer(span, &mut buffer);
            }
        });
    }

    pub fn write_log_batch(&self, batch: &[SdkLogRecord]) {
        self.with_writer(|w| {
            let mut buffer = BufWriter::new(w);
            for log_data in batch {
                let _ = self.log_to_writer(log_data, &mut buffer);
            }
        });
    }

    fn with_writer<R>(&self, f: impl FnOnce(&mut dyn Write) -> R) -> R {
        match &self.options.target {
            Target::Stdout => f(&mut io::stdout()),
            Target::Stderr => f(&mut io::stderr()),
            Target::Pipe(p) => f(&mut *p.lock().expect("pipe lock poisoned")),
        }
    }

    fn span_to_writer<W: io::Write>(&self, span: &SpanData, w: &mut W) -> io::Result<()> {
        // only print for pending span and logs
        if span.get_span_type().is_none_or(|ty| ty == "span") {
            return Ok(());
        }

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
                    if let Value::I64(level_num) = kv.value {
                        if level_num < tracing_level_to_severity(self.options.min_log_level) as i64
                        {
                            return Ok(());
                        }
                        level = Some(level_num);
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

        if self.options.include_timestamps {
            let timestamp: DateTime<Utc> = span.start_time.into();
            write!(
                w,
                "{}",
                DIMMED.paint(timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
            )?;
        }

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

    fn log_to_writer<W: io::Write>(&self, log_record: &SdkLogRecord, w: &mut W) -> io::Result<()> {
        let mut msg = None;
        let mut target = None;

        let mut fields = Vec::new();

        for (key, value) in log_record.attributes_iter() {
            match key.as_str() {
                "logfire.msg" => {
                    if let opentelemetry::logs::AnyValue::String(s) = value {
                        msg = Some(s.as_str());
                    }
                }
                "code.namespace" => {
                    if let opentelemetry::logs::AnyValue::String(s) = value {
                        target = Some(s.as_str());
                    }
                }
                // Filter out known values
                "logfire.json_schema"
                | "code.filepath"
                | "code.lineno"
                | "thread.id"
                | "thread.name"
                | "logfire.null_args"
                | "busy_ns"
                | "idle_ns" => (),
                _ => {
                    fields.push((key, value));
                }
            }
        }

        if msg.is_none() {
            // Use the body as the message if no logfire.msg
            if let Some(AnyValue::String(s)) = log_record.body() {
                msg = Some(s.as_str());
            }
        }

        if self.options.include_timestamps {
            if let Some(timestamp) = log_record.timestamp() {
                let timestamp: DateTime<Utc> = timestamp.into();
                write!(
                    w,
                    "{}",
                    DIMMED.paint(timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
                )?;
            }
        }

        if let Some(level) = log_record.severity_number() {
            level_int_to_text(level as i64, w)?;
        }

        if let Some(target) = target {
            write!(w, " {}", DIMMED_AND_ITALIC.paint(target))?;
        }

        if let Some(msg) = msg {
            write!(w, " {}", BOLD.paint(msg))?;
        }

        if !fields.is_empty() {
            for (idx, (key, value)) in fields.iter().enumerate() {
                let key = key.as_str();
                let value_str = match value {
                    opentelemetry::logs::AnyValue::String(s) => s.as_str(),
                    opentelemetry::logs::AnyValue::Int(i) => {
                        return write!(w, " {}={}", ITALIC.paint(key), i);
                    }
                    opentelemetry::logs::AnyValue::Double(d) => {
                        return write!(w, " {}={}", ITALIC.paint(key), d);
                    }
                    opentelemetry::logs::AnyValue::Boolean(b) => {
                        return write!(w, " {}={}", ITALIC.paint(key), b);
                    }
                    _ => &format!("{value:?}"),
                };
                write!(w, " {}={}", ITALIC.paint(key), value_str)?;
                if idx < fields.len() - 1 {
                    write!(w, ",")?;
                }
            }
        }

        writeln!(w)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        config::{ConsoleOptions, Target},
        set_local_logfire,
        test_utils::remap_timestamps_in_console_output,
    };
    use insta::assert_snapshot;
    use tracing::{Level, level_filters::LevelFilter};

    #[test]
    fn test_print_to_console() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions::default()
            .with_target(Target::Pipe(output.clone()))
            .with_min_log_level(Level::TRACE);

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = set_local_logfire(logfire);

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tracing::subscriber::with_default(guard.subscriber().clone(), || {
                let root = crate::span!("root span").entered();
                let _ = crate::span!("hello world span").entered();
                let _ = crate::span!(level: Level::DEBUG, "debug span");
                let _ = crate::span!(parent: &root, level: Level::DEBUG, "debug span with explicit parent");
                crate::info!("hello world log");
                panic!("oh no!");
            });
        }))
        .unwrap_err();

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();
        let output = remap_timestamps_in_console_output(output);

        assert_snapshot!(output, @r"
        [2m1970-01-01T00:00:00.000000Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mroot span[0m
        [2m1970-01-01T00:00:00.000001Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world span[0m
        [2m1970-01-01T00:00:00.000002Z[0m[34m DEBUG[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mdebug span[0m
        [2m1970-01-01T00:00:00.000003Z[0m[34m DEBUG[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mdebug span with explicit parent[0m
        [2m1970-01-01T00:00:00.000004Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world log[0m
        [2m1970-01-01T00:00:00.000005Z[0m[31m ERROR[0m [1mpanic: oh no![0m [3mbacktrace[0m=disabled backtrace
        ");
    }

    #[test]
    fn test_print_to_console_include_timestamps_false() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions::default()
            .with_target(Target::Pipe(output.clone()))
            .with_include_timestamps(false)
            .with_min_log_level(Level::TRACE);

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = set_local_logfire(logfire);

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tracing::subscriber::with_default(guard.subscriber().clone(), || {
                let root = crate::span!("root span").entered();
                let _ = crate::span!("hello world span").entered();
                let _ = crate::span!(level: Level::DEBUG, "debug span");
                let _ = crate::span!(parent: &root, level: Level::DEBUG, "debug span with explicit parent");
                crate::info!("hello world log");
                panic!("oh no!");
            });
        }))
        .unwrap_err();

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();
        let output = remap_timestamps_in_console_output(output);

        assert_snapshot!(output, @r"
        [32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mroot span[0m
        [32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world span[0m
        [34m DEBUG[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mdebug span[0m
        [34m DEBUG[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mdebug span with explicit parent[0m
        [32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world log[0m
        [31m ERROR[0m [1mpanic: oh no![0m [3mbacktrace[0m=disabled backtrace
        ");
    }

    #[test]
    fn test_print_to_console_with_min_log_level() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions::default()
            .with_target(Target::Pipe(output.clone()))
            .with_min_log_level(Level::INFO);

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = set_local_logfire(logfire);

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tracing::subscriber::with_default(guard.subscriber().clone(), || {
                let root = crate::span!("root span").entered();
                let _ = crate::span!("hello world span").entered();
                let _ = crate::span!(level: Level::DEBUG, "debug span");
                let _ = crate::span!(parent: &root, level: Level::DEBUG, "debug span with explicit parent");
                crate::info!("hello world log");
                panic!("oh no!");
            });
        }))
        .unwrap_err();

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();
        let output = remap_timestamps_in_console_output(output);

        assert_snapshot!(output, @r"
        [2m1970-01-01T00:00:00.000000Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mroot span[0m
        [2m1970-01-01T00:00:00.000001Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world span[0m
        [2m1970-01-01T00:00:00.000002Z[0m[32m  INFO[0m [2;3mlogfire::internal::exporters::console::tests[0m [1mhello world log[0m
        [2m1970-01-01T00:00:00.000003Z[0m[31m ERROR[0m [1mpanic: oh no![0m [3mbacktrace[0m=disabled backtrace
        ");
    }

    /// Regression test for https://github.com/pydantic/logfire-rust/issues/46
    #[test]
    fn test_console_deadlock() {
        let shutdown_handler = crate::configure()
            .send_to_logfire(false)
            .local()
            .install_panic_handler()
            .finish()
            .unwrap();

        let guard = set_local_logfire(shutdown_handler.clone());

        // Why did this deadlock?
        //
        // - calling `crate::info!` would use `SimpleSpanProcessor` to record the span
        // - `SimpleSpanProcessor` had a mutex, which it locked, and then used `block_on` internally to call the exporter
        // - `block_on` would panic, which caused another span to be emitted
        // - this then deadlocked inside `SimpleSpanProcessor` because it was already holding the mutex
        futures::executor::block_on(async {
            crate::info!("Testing console output with tokio sleep");
        });

        drop(guard);

        shutdown_handler.shutdown().ok();
    }
}
