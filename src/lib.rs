//! # Rust SDK for Pydantic Logfire
//!
//! This goal of this SDK is to provide a first-class experience instrumenting Rust code for the Pydantic Logfire platform.
//!
//! The most important API is [`logfire::configure()`][configure], which is used to set up
//! integrations with `tracing` and `log`, as well as exporters for `opentelemetry`. Code
//! instrumented using `tracing` and `log` will *just work* once this configuration is in place.
//!
//! This SDK also offers opinionated functions to instrument code following Logfire's design principles:
//!   - [`logfire::info!()`][info], [`logfire::debug!()`][debug] and similar macros to log messages
//!     with structured data.
//!   - [`logfire::span!()`][span] to create a span with structured data.
//!   - [`logfire::u64_counter()`][u64_counter] and similar functions to create metrics.
//!
//! See also:
//!  - The [integrations](#integrations) section below for more information on the relationship of
//!    this SDK to other libraries.
//!  - The [Logfire documentation](https://logfire.pydantic.dev/docs/) for more information about Logfire in general.
//!  - The [Logfire GitHub repository](https://github.com/pydantic/logfire) for the source of the documentation, the Python SDK and an issue tracker for general questions about Logfire.
//!
//! ## Getting Started
//!
//! To use Logfire in your Rust project, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
#![doc = concat!("logfire = \"", env!("CARGO_PKG_VERSION"), "\"\n")]
//! ```
//!
//! Then, in your Rust code, add a call to `logfire::configure()` at the beginning of your program:
//!
//! ```rust
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let shutdown_handler = logfire::configure()
//!         .install_panic_handler()
//! #        .send_to_logfire(logfire::SendToLogfire::IfTokenPresent)
//!         .finish()?;
//!
//!     logfire::info!("Hello, world!");
//!
//!     shutdown_handler.shutdown()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration
//!
//! ## Integrations
//!
//! ### With `tracing`
//!
//! ### With `opentelemetry`
//!
//! ### With `log`

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::panic::PanicHookInfo;
use std::sync::{Arc, LazyLock, Once};
use std::{backtrace::Backtrace, env::VarError, str::FromStr, sync::OnceLock, time::Duration};

use bridges::tracing::LogfireTracingLayer;
use chrono::{DateTime, Utc};
use futures_util::future::BoxFuture;
use internal::constants::ATTRIBUTES_SPAN_TYPE_KEY;
use internal::exporters::remove_pending::RemovePendingSpansExporter;
use nu_ansi_term::{Color, Style};
use opentelemetry::Value;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{
    MetricExporter, Protocol, SpanExporter, WithExportConfig, WithHttpConfig,
};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, SimpleSpanProcessor};
use opentelemetry_sdk::trace::{SdkTracerProvider, SpanData, Tracer};
use thiserror::Error;
use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

mod bridges;
mod config;
mod macros;
mod metrics;
mod ulid_id_generator;

pub use config::*;
pub use macros::*;
pub use metrics::*;
use ulid_id_generator::UlidIdGenerator;

mod internal;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigureError {
    /// Logfire has already been configured
    #[error("Logfire has already been configured")]
    AlreadyConfigured,

    /// Error configuring the `log::logger`.
    #[error("Error configuring the global logger: {0}")]
    Logging(#[from] log::SetLoggerError),

    /// Error configuring the OpenTelemetry metrics.
    #[error("Error configuring the OpenTelemetry metrics: {0}")]
    Metrics(#[from] opentelemetry_sdk::metrics::MetricError),

    /// Error configuring the OpenTelemetry tracer.
    #[error("Error configuring the OpenTelemetry tracer: {0}")]
    Trace(#[from] opentelemetry::trace::TraceError),

    /// Error installing the OpenTelemetry tracer.
    #[error("Error configuring the OpenTelemetry tracer: {0}")]
    TracingAlreadySetup(#[from] tracing::subscriber::SetGlobalDefaultError),

    /// Error parsing the RUST_LOG environment variable.
    #[error("Error configuring the OpenTelemetry tracer: {0}")]
    RustLogInvalid(#[from] tracing_subscriber::filter::FromEnvError),

    #[error("Rust feature required: `{feature_name}` feature must be enabled for {functionality}")]
    LogfireFeatureRequired {
        feature_name: &'static str,
        functionality: String,
    },

    #[error("Invalid configuration value for {parameter}: {value}")]
    InvalidConfigurationValue {
        parameter: &'static str,
        value: String,
    },

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ConsoleMode {
    /// Write to console if no logfire token
    #[default]
    Fallback,
    /// Force write to console
    Force,
}

impl FromStr for ConsoleMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fallback" => Ok(ConsoleMode::Fallback),
            "force" => Ok(ConsoleMode::Force),
            _ => Err(format!("invalid console mode: {s}")),
        }
    }
}

pub struct LogfireConfigBuilder {
    send_to_logfire: Option<SendToLogfire>,
    console_mode: ConsoleMode,
    install_panic_handler: bool,
    default_level_filter: Option<LevelFilter>,
    tracer_provider: Option<SdkTracerProvider>,
}

#[must_use]
pub fn configure() -> LogfireConfigBuilder {
    LogfireConfigBuilder {
        send_to_logfire: None,
        console_mode: ConsoleMode::Fallback,
        install_panic_handler: false,
        default_level_filter: None,
        tracer_provider: None,
    }
}

impl LogfireConfigBuilder {
    pub fn install_panic_handler(&mut self) -> &mut Self {
        self.install_panic_handler = true;
        self
    }

    /// Whether to send data to the Logfire platform.
    ///
    /// Defaults to the value of `LOGFIRE_SEND_TO_LOGFIRE` if set, otherwise `Yes`.
    pub fn send_to_logfire(&mut self, send_to_logfire: SendToLogfire) -> &mut Self {
        self.send_to_logfire = Some(send_to_logfire);
        self
    }
    pub fn console_mode(&mut self, console_mode: ConsoleMode) -> &mut Self {
        self.console_mode = console_mode;
        self
    }
    pub fn with_tracer_provider(&mut self, tracer_provider: SdkTracerProvider) -> &mut Self {
        self.tracer_provider = Some(tracer_provider);
        self
    }
    pub fn with_defalt_level_filter(&mut self, default_level_filter: LevelFilter) -> &mut Self {
        self.default_level_filter = Some(default_level_filter);
        self
    }

    /// Finish configuring Logfire.
    ///
    /// # Errors
    ///
    /// See [`ConfigureError`] for possible errors.
    pub fn finish(&self) -> Result<ShutdownHandler, ConfigureError> {
        let LogfireParts {
            tracer,
            subscriber,
            tracer_provider,
            logfire_token,
            send_to_logfire,
        } = self.build_parts(None)?;

        tracing::subscriber::set_global_default(subscriber)?;
        let logger = bridges::log::LogfireLogger::init(tracer.inner.clone());
        log::set_logger(logger)?;
        log::set_max_level(logger.max_level());

        GLOBAL_TRACER
            .set(tracer)
            .map_err(|_| ConfigureError::AlreadyConfigured)?;

        let propagator = opentelemetry::propagation::TextMapCompositePropagator::new(vec![
            Box::new(opentelemetry_sdk::propagation::TraceContextPropagator::new()),
            Box::new(opentelemetry_sdk::propagation::BaggagePropagator::new()),
        ]);
        opentelemetry::global::set_text_map_propagator(propagator);

        // setup metrics only if sending to logfire
        let meter_provider = if send_to_logfire {
            let metric_reader =
                PeriodicReader::builder(metric_exporter(logfire_token.as_deref())?).build();

            let meter_provider = SdkMeterProvider::builder()
                .with_reader(metric_reader)
                .build();

            opentelemetry::global::set_meter_provider(meter_provider.clone());

            Some(meter_provider)
        } else {
            None
        };

        Ok(ShutdownHandler {
            tracer_provider,
            meter_provider,
        })
    }

    fn build_parts(
        &self,
        env: Option<HashMap<String, String>>,
    ) -> Result<LogfireParts, ConfigureError> {
        let logfire_token = get_optional_env("LOGFIRE_TOKEN", env.as_ref())?;

        let send_to_logfire = match self.send_to_logfire {
            Some(send_to_logfire) => send_to_logfire,
            None => match get_optional_env("LOGFIRE_SEND_TO_LOGFIRE", env.as_ref())? {
                Some(value) => value.parse()?,
                None => SendToLogfire::Yes,
            },
        };

        let send_to_logfire = match send_to_logfire {
            SendToLogfire::Yes => true,
            SendToLogfire::IfTokenPresent => logfire_token.is_some(),
            SendToLogfire::No => false,
        };

        let tracer_provider = if let Some(provider) = &self.tracer_provider {
            provider.clone()
        } else {
            let tracer_provider_builder =
                SdkTracerProvider::builder().with_id_generator(UlidIdGenerator::new());

            if send_to_logfire {
                tracer_provider_builder.with_span_processor(
                    BatchSpanProcessor::builder(RemovePendingSpansExporter::new(
                        LogfireSpanExporter {
                            write_console: self.console_mode == ConsoleMode::Force,
                            inner: Some(span_exporter(logfire_token.as_deref())?),
                        },
                    ))
                    .with_batch_config(
                        BatchConfigBuilder::default()
                            .with_scheduled_delay(Duration::from_millis(500)) // 500 matches Python
                            .build(),
                    )
                    .build(),
                )
            } else {
                tracer_provider_builder.with_span_processor(SimpleSpanProcessor::new(Box::new(
                    LogfireSpanExporter {
                        write_console: true,
                        inner: None,
                    },
                )))
            }
            .build()
        };

        let tracer = tracer_provider.tracer("logfire");
        let default_level_filter = self.default_level_filter.unwrap_or(if send_to_logfire {
            // by default, send everything to the logfire platform, for best UX
            LevelFilter::TRACE
        } else {
            // but if printing locally, just set INFO
            LevelFilter::INFO
        });

        let filter = tracing_subscriber::EnvFilter::builder()
            .with_default_directive(default_level_filter.into())
            .from_env()?; // but allow the user to override this with `RUST_LOG`

        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_opentelemetry::layer()
                    .with_error_records_to_exceptions(true)
                    .with_tracer(tracer.clone()),
            )
            .with(LogfireTracingLayer(tracer.clone()));

        if self.install_panic_handler {
            install_panic_handler();
        }

        Ok(LogfireParts {
            tracer: LogfireTracer {
                inner: tracer,
                handle_panics: self.install_panic_handler,
            },
            subscriber: Arc::new(subscriber),
            tracer_provider,
            logfire_token,
            send_to_logfire,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ShutdownHandler {
    tracer_provider: SdkTracerProvider,
    meter_provider: Option<SdkMeterProvider>,
}

impl ShutdownHandler {
    /// Shutdown the tracer provider.
    ///
    /// This will flush all spans and metrics to the exporter.
    ///
    /// # Errors
    ///
    /// See [`ConfigureError`] for possible errors.
    pub fn shutdown(&self) -> Result<(), ConfigureError> {
        // TODO: can this just be done in Drop?
        self.tracer_provider
            .shutdown()
            .map_err(|e| ConfigureError::Other(e.into()))?;
        if let Some(meter_provider) = &self.meter_provider {
            meter_provider
                .shutdown()
                .map_err(|e| ConfigureError::Other(e.into()))?;
        }
        Ok(())
    }
}

struct LogfireParts {
    tracer: LogfireTracer,
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    tracer_provider: SdkTracerProvider,
    logfire_token: Option<String>,
    send_to_logfire: bool,
}

/// Install `handler` as part of a chain of panic handlers.
fn install_panic_handler() {
    fn panic_hook(info: &PanicHookInfo) {
        if try_with_logfire_tracer(|tracer| tracer.handle_panics) != Some(true) {
            // this tracer is not handling panics
            return;
        }

        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s
        } else {
            ""
        };

        crate::error!(
            "panic: {message}",
            location = info.location().as_ref().map(ToString::to_string),
            backtrace = Backtrace::capture().to_string(),
        );
    }

    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            panic_hook(info);
            prev(info);
        }));
    });
}

/// Internal helper to get the `key` from the environment.
///
/// If `env` is provided, will use that instead of the process environment.
fn get_optional_env(
    key: &str,
    env: Option<&HashMap<String, String>>,
) -> Result<Option<String>, ConfigureError> {
    if let Some(env) = env {
        Ok(env.get(key).cloned())
    } else {
        match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(VarError::NotPresent) => Ok(None),
            Err(VarError::NotUnicode(_)) => Err(ConfigureError::Other(
                format!("{key} is not valid UTF-8").into(),
            )),
        }
    }
}

/// Simple span exporter which attempts to match the "simple" Python logfire console exporter.
#[derive(Debug)]
struct LogfireSpanExporter {
    write_console: bool,
    inner: Option<SpanExporter>,
}

impl opentelemetry_sdk::trace::SpanExporter for LogfireSpanExporter {
    fn export(
        &mut self,
        batch: Vec<opentelemetry_sdk::trace::SpanData>,
    ) -> BoxFuture<'static, opentelemetry_sdk::error::OTelSdkResult> {
        if self.write_console {
            for span in &batch {
                let mut buffer = String::new();
                if Self::print_span(span, &mut buffer).is_ok() && !buffer.is_empty() {
                    println!("{buffer}");
                };
            }
        }
        if let Some(inner) = &mut self.inner {
            inner.export(batch)
        } else {
            Box::pin(async { Ok(()) })
        }
    }

    fn shutdown(&mut self) -> opentelemetry_sdk::error::OTelSdkResult {
        if let Some(inner) = &mut self.inner {
            inner.shutdown()
        } else {
            Ok(())
        }
    }

    fn force_flush(&mut self) -> opentelemetry_sdk::error::OTelSdkResult {
        if let Some(inner) = &mut self.inner {
            inner.force_flush()
        } else {
            Ok(())
        }
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        if let Some(inner) = &mut self.inner {
            inner.set_resource(resource);
        }
    }
}

static DIMMED: LazyLock<Style> = LazyLock::new(|| Style::new().dimmed());
static DIMMED_AND_ITALIC: LazyLock<Style> = LazyLock::new(|| DIMMED.italic());
static BOLD: LazyLock<Style> = LazyLock::new(|| Style::new().bold());
static ITALIC: LazyLock<Style> = LazyLock::new(|| Style::new().italic());

fn level_int_to_text<W: fmt::Write>(level: i64, w: &mut W) -> fmt::Result {
    match level {
        1 => write!(w, "{}", Color::Purple.paint(" TRACE")),
        2..=5 => write!(w, "{}", Color::Blue.paint(" DEBUG")),
        6..=9 => write!(w, "{}", Color::Green.paint("  INFO")),
        10..=13 => write!(w, "{}", Color::Yellow.paint("  WARN")),
        14.. => write!(w, "{}", Color::Red.paint(" ERROR")),
        _ => write!(w, "{}", Color::DarkGray.paint(" -----")),
    }
}

impl LogfireSpanExporter {
    fn print_span<W: fmt::Write>(span: &SpanData, w: &mut W) -> fmt::Result {
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
        if span_type == "pending_span" {
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

        Ok(())
    }
}

// current default logfire protocol is to export over HTTP in binary format
const DEFAULT_LOGFIRE_PROTOCOL: Protocol = Protocol::HttpBinary;

// standard OTLP protocol values in configuration
const OTEL_EXPORTER_OTLP_PROTOCOL_GRPC: &str = "grpc";
const OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF: &str = "http/protobuf";
const OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_JSON: &str = "http/json";

/// Temporary workaround for lack of https://github.com/open-telemetry/opentelemetry-rust/pull/2758
fn protocol_from_str(value: &str) -> Result<Protocol, ConfigureError> {
    match value {
        OTEL_EXPORTER_OTLP_PROTOCOL_GRPC => Ok(Protocol::Grpc),
        OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF => Ok(Protocol::HttpBinary),
        OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_JSON => Ok(Protocol::HttpJson),
        _ => Err(ConfigureError::Other(
            format!("unsupported protocol: {value}").into(),
        )),
    }
}

/// Get a protocol from the environment (or default value), returning a string describing the source
/// plus the parsed protocol.
fn protocol_from_env(data_env_var: &str) -> Result<(String, Protocol), ConfigureError> {
    // try both data-specific env var and general protocol
    [data_env_var, "OTEL_EXPORTER_OTLP_PROTOCOL"]
        .into_iter()
        .find_map(|var_name| match get_optional_env(var_name, None) {
            Ok(Some(value)) => Some(Ok((var_name, value))),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .transpose()?
        .map_or_else(
            || {
                Ok((
                    "the default logfire export protocol".to_string(),
                    DEFAULT_LOGFIRE_PROTOCOL,
                ))
            },
            |(var_name, value)| Ok((format!("`{var_name}={value}`"), protocol_from_str(&value)?)),
        )
}

macro_rules! feature_required {
    ($feature_name:literal, $functionality:expr, $if_enabled:expr) => {{
        #[cfg(feature = $feature_name)]
        {
            Ok($if_enabled)
        }

        #[cfg(not(feature = $feature_name))]
        {
            Err(ConfigureError::LogfireFeatureRequired {
                feature_name: $feature_name,
                functionality: $functionality,
            })
        }
    }};
}

fn span_exporter(logfire_token: Option<&str>) -> Result<SpanExporter, ConfigureError> {
    let (source, protocol) = protocol_from_env("OTEL_EXPORTER_OTLP_TRACES_PROTOCOL")?;

    let builder = SpanExporter::builder();

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_TRACES_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    match protocol {
        Protocol::Grpc => {
            feature_required!("export-grpc", source, { builder.with_tonic().build()? })
        }
        Protocol::HttpBinary => {
            feature_required!("export-http-protobuf", source, {
                builder
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_logfire_http_defaults(logfire_token, "v1/traces")
                    .build()?
            })
        }
        Protocol::HttpJson => {
            feature_required!("export-http-json", source, {
                builder
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_logfire_http_defaults(logfire_token, "v1/traces")
                    .build()?
            })
        }
    }
}

fn metric_exporter(logfire_token: Option<&str>) -> Result<MetricExporter, ConfigureError> {
    let (source, protocol) = protocol_from_env("OTEL_EXPORTER_OTLP_METRICS_PROTOCOL")?;

    let builder =
        MetricExporter::builder().with_temporality(opentelemetry_sdk::metrics::Temporality::Delta);

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_METRICS_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    match protocol {
        Protocol::Grpc => {
            feature_required!("export-grpc", source, { builder.with_tonic().build()? })
        }
        Protocol::HttpBinary => {
            feature_required!("export-http-protobuf", source, {
                builder
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_logfire_http_defaults(logfire_token, "v1/metrics")
                    .build()?
            })
        }
        Protocol::HttpJson => {
            feature_required!("export-http-json", source, {
                builder
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_logfire_http_defaults(logfire_token, "v1/metrics")
                    .build()?
            })
        }
    }
}

/// Internal helper to build an exporter with logfire default config
trait WithLogfireHttpExportDefaults: WithHttpConfig + WithExportConfig + Sized {
    fn with_logfire_http_defaults(self, logfire_token: Option<&str>, endpoint: &str) -> Self;
}

impl<T> WithLogfireHttpExportDefaults for T
where
    T: WithHttpConfig + WithExportConfig + Sized,
{
    fn with_logfire_http_defaults(self, logfire_token: Option<&str>, endpoint: &str) -> Self {
        let mut headers = HashMap::new();
        if let Some(logfire_token) = logfire_token {
            headers.insert(
                "Authorization".to_string(),
                format!("Bearer {logfire_token}"),
            );
        }
        self.with_headers(headers)
            .with_endpoint(format!("https://logfire-api.pydantic.dev/{endpoint}"))
    }
}

struct LogfireTracer {
    inner: Tracer,
    handle_panics: bool,
}

// Global tracer configured in `logfire::configure()`
static GLOBAL_TRACER: OnceLock<LogfireTracer> = OnceLock::new();

thread_local! {
    static LOCAL_TRACER: RefCell<Option<LogfireTracer>> = const { RefCell::new(None) };
}

/// Internal function to execute some code with the current tracer
fn try_with_logfire_tracer<R>(f: impl FnOnce(&LogfireTracer) -> R) -> Option<R> {
    let mut f = Some(f);
    if let Some(result) = LOCAL_TRACER
        .try_with(|local_logfire| {
            local_logfire
                .borrow()
                .as_ref()
                .map(|tracer| f.take().expect("not called")(tracer))
        })
        .ok()
        .flatten()
    {
        return Some(result);
    }

    GLOBAL_TRACER.get().map(f.expect("local tls not used"))
}

/// Helper for installing a logfire guard locally to a thread.
///
/// This is a bit of a mess, it's only implemented far enough to make tests pass...
#[cfg(test)]
struct LocalLogfireGuard {
    prior: Option<LogfireTracer>,
    /// Tracing subscriber which can be used to subscribe tracing
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    /// Shutdown handler
    #[allow(dead_code)]
    shutdown_handler: ShutdownHandler,
}

#[cfg(test)]
impl Drop for LocalLogfireGuard {
    fn drop(&mut self) {
        // FIXME: if drop order is not consistent with creation order, does this create strange
        // state?
        LOCAL_TRACER.with_borrow_mut(|local_logfire| {
            *local_logfire = self.prior.take();
        });
    }
}

#[cfg(test)]
#[expect(clippy::needless_pass_by_value)] // might consume in the future, leave it for now
fn set_local_logfire(config: LogfireConfigBuilder) -> Result<LocalLogfireGuard, ConfigureError> {
    let LogfireParts {
        tracer,
        subscriber,
        tracer_provider,
        ..
    } = config.build_parts(None)?;

    let prior = LOCAL_TRACER.with_borrow_mut(|local_logfire| local_logfire.replace(tracer));

    // TODO: logs??
    // TODO: metrics??

    Ok(LocalLogfireGuard {
        prior,
        subscriber,
        shutdown_handler: ShutdownHandler {
            tracer_provider,
            meter_provider: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, hash_map::Entry},
        future::Future,
        pin::Pin,
        sync::atomic::{AtomicU64, Ordering},
        time::SystemTime,
    };

    use insta::assert_debug_snapshot;
    use opentelemetry::trace::{SpanId, TraceId};
    use opentelemetry_sdk::{
        error::OTelSdkResult,
        trace::{
            IdGenerator, InMemorySpanExporterBuilder, SdkTracerProvider, SpanData, SpanExporter,
        },
    };
    use tracing::Level;

    use super::*;

    #[derive(Debug)]
    pub struct DeterministicIdGenerator {
        next_trace_id: AtomicU64,
        next_span_id: AtomicU64,
    }

    impl IdGenerator for DeterministicIdGenerator {
        fn new_trace_id(&self) -> opentelemetry::trace::TraceId {
            TraceId::from_u128(self.next_trace_id.fetch_add(1, Ordering::Relaxed).into())
        }

        fn new_span_id(&self) -> opentelemetry::trace::SpanId {
            SpanId::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed))
        }
    }

    impl DeterministicIdGenerator {
        pub fn new() -> Self {
            // start at OxF0 because 0 is reserved for invalid IDs,
            // and if we have a couple of bytes used, it's a more interesting check of
            // the hex formatting
            Self {
                next_trace_id: 0xF0.into(),
                next_span_id: 0xF0.into(),
            }
        }
    }

    #[derive(Debug)]
    pub struct DeterministicExporter<Inner> {
        exporter: Inner,
        next_timestamp: u64,
        timestamp_remap: HashMap<SystemTime, SystemTime>,
    }

    impl<Inner: SpanExporter> SpanExporter for DeterministicExporter<Inner> {
        fn export(
            &mut self,
            mut batch: Vec<SpanData>,
        ) -> Pin<Box<dyn Future<Output = OTelSdkResult> + Send>> {
            for span in &mut batch {
                // By remapping timestamps to deterministic values, we should find that
                // - pending spans have the same start time as their real span
                // - pending spans also have the same end as start
                span.start_time = self.remap_timestamp(span.start_time);
                span.end_time = self.remap_timestamp(span.end_time);

                // thread info is not deterministic
                // nor are timings
                for attr in &mut span.attributes {
                    if attr.key.as_str() == "thread.id"
                        || attr.key.as_str() == "busy_ns"
                        || attr.key.as_str() == "idle_ns"
                    {
                        attr.value = 0.into();
                    }
                }
            }
            self.exporter.export(batch)
        }
    }

    impl<Inner> DeterministicExporter<Inner> {
        pub fn new(exporter: Inner) -> Self {
            Self {
                exporter,
                next_timestamp: 0,
                timestamp_remap: HashMap::new(),
            }
        }

        fn remap_timestamp(&mut self, from: SystemTime) -> SystemTime {
            match self.timestamp_remap.entry(from) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let new_timestamp = SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(self.next_timestamp);
                    self.next_timestamp += 1;
                    *entry.insert(new_timestamp)
                }
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    #[test]
    fn test_basic_span() {
        let exporter = InMemorySpanExporterBuilder::new().build();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(DeterministicExporter::new(exporter.clone()))
            .with_id_generator(DeterministicIdGenerator::new())
            .build();

        let mut config = crate::configure();
        config
            .send_to_logfire(SendToLogfire::No)
            .install_panic_handler()
            .with_tracer_provider(provider)
            .with_defalt_level_filter(LevelFilter::TRACE);

        let guard = set_local_logfire(config).unwrap();

        // let tracer = provider.tracer("test");

        // let subscriber = tracing_subscriber::registry()
        //     .with(
        //         tracing_opentelemetry::layer()
        //             .with_error_records_to_exceptions(true)
        //             .with_tracer(tracer.clone()),
        //     )
        //     .with(LogfireTracingLayer(tracer.clone()));

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tracing::subscriber::with_default(guard.subscriber.clone(), || {
                let root = span!("root span").entered();
                let _ = span!("hello world span").entered();
                let _ = span!(level: Level::DEBUG, "debug span");
                let _ =
                    span!(parent: &root, level: Level::DEBUG, "debug span with explicit parent");
                info!("hello world log");
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
                                "src/lib.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            964,
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
                                "tests::test_basic_span",
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
                                "src/lib.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            965,
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
                                "tests::test_basic_span",
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
                                "src/lib.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            965,
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
                                "tests::test_basic_span",
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
                                "src/lib.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            966,
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
                                "tests::test_basic_span",
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
                                "src/lib.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            968,
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
                                "tests::test_basic_span",
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
                                "{\"type\":\"object\",\"properties\":{}}",
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
                            969,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
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
                                "tests::test_basic_span",
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
                                "src/lib.rs:970:17",
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
                            401,
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
                                "tests::test_basic_span",
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
                                "src/lib.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            964,
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
                                "tests::test_basic_span",
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

    #[test]
    fn test_send_to_logfire() {
        for (env, setting, expected) in [
            (vec![], None, true),
            (vec![("LOGFIRE_SEND_TO_LOGFIRE", "no")], None, false),
            (vec![("LOGFIRE_SEND_TO_LOGFIRE", "yes")], None, true),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "if-token-present")],
                None,
                false,
            ),
            (
                vec![
                    ("LOGFIRE_SEND_TO_LOGFIRE", "if-token-present"),
                    ("LOGFIRE_TOKEN", "a"),
                ],
                None,
                true,
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no")],
                Some(SendToLogfire::Yes),
                true,
            ),
        ] {
            let env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();

            let mut config = crate::configure();
            if let Some(value) = setting {
                config.send_to_logfire(value);
            }

            let parts = config.build_parts(Some(env)).unwrap();

            assert_eq!(parts.send_to_logfire, expected);
        }
    }
}
