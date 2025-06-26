use std::{
    io::{self, BufWriter, Write},
    sync::{Arc, LazyLock, Mutex, mpsc},
};

use chrono::{DateTime, Utc};
use nu_ansi_term::{Color, Style};
use opentelemetry::Value;
use opentelemetry_sdk::trace::SpanData;

use crate::{
    bridges::tracing::level_to_level_number,
    config::{ConsoleOptions, Target},
    internal::{constants::ATTRIBUTES_SPAN_TYPE_KEY, span_data_ext::SpanDataExt},
};

/// Simple span processor which attempts to match the "simple" Python logfire console exporter.
#[derive(Debug)]
pub struct SimpleConsoleSpanProcessor {
    tx: Mutex<Option<mpsc::Sender<SpanData>>>,
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl SimpleConsoleSpanProcessor {
    pub fn new(writer: Arc<ConsoleWriter>) -> Self {
        let (tx, rx) = mpsc::channel();

        // Spawn a thread to handle the received spans
        let thread_handle = std::thread::spawn(move || {
            while let Ok(span) = rx.recv() {
                writer.write_batch(&[span]);
            }
        });

        Self {
            tx: Mutex::new(Some(tx)),
            thread_handle: Mutex::new(Some(thread_handle)),
        }
    }
}

impl opentelemetry_sdk::trace::SpanProcessor for SimpleConsoleSpanProcessor {
    fn on_start(&self, _span: &mut opentelemetry_sdk::trace::Span, _cx: &opentelemetry::Context) {}

    fn on_end(&self, span: opentelemetry_sdk::trace::SpanData) {
        if let Some(tx) = self.tx.lock().expect("no poisoning").as_mut() {
            tx.send(span)
                .expect("failed to send span to console writer");
        }
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(
        &self,
        _timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        // FIXME should timeout here?
        *self.tx.lock().expect("no poisoning") = None;
        if let Some(thread_handle) = self.thread_handle.lock().expect("no poisoning").take() {
            thread_handle.join().expect("failed to join thread");
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
                        if level_num < level_to_level_number(self.options.min_log_level) {
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
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use insta::assert_snapshot;
    use tracing::{Level, level_filters::LevelFilter};

    use crate::{
        config::{ConsoleOptions, Target},
        set_local_logfire,
        test_utils::remap_timestamps_in_console_output,
    };

    #[test]
    fn test_print_to_console() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions::default()
            .with_target(Target::Pipe(output.clone()))
            .with_min_log_level(Level::TRACE);

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = set_local_logfire(handler);

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

        guard.shutdown_handler.shutdown().unwrap();

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

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = set_local_logfire(handler);

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

        guard.shutdown_handler.shutdown().unwrap();

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

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = set_local_logfire(handler);

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

        guard.shutdown_handler.shutdown().unwrap();

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
