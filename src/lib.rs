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
//! > ***Initial release - feedback wanted!***
//! >
//! > This is an initial release of the Logfire Rust SDK. We've been using it internally to build
//! > Logfire for some time, and it is serving us well. As we're using it ourselves in production,
//! > we figured it's ready for everyone else also using Logfire.
//! >
//! > We are continually iterating to make this SDK better. We'd love your feedback on all aspects
//! > of the SDK and are keen to make the design as idiomatic and performant as possible. There are
//! > also many features currently supported by the Python SDK which are not yet supported by this
//! > SDK; please open issues to help us prioritize these to close this gap. For example, we have not
//! > yet implemented scrubbing in this Rust SDK, although we are aware it is important!
//! >
//! > In particular, the current coupling to `tracing` is an open design point. By building on top
//! > of tracing we get widest compatibility and a relatively simple SDK, however to make
//! > Logfire-specific adjustments we might prefer in future to move `tracing` to be an optional
//! > integration.
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
#![doc = include_str!("../examples/basic.rs")]
//! ```
//!
//! ## Configuration
//!
//! After adding basic setup as per above, the most two important environment variables are:
//! - `LOGFIRE_TOKEN` - (required) the token to send data to the Logfire platform
//! - `RUST_LOG` - (optional) the level of verbosity to send to the Logfire platform. By default
//!   logs are captured at `TRACE` level so that all data is available for you to analyze in the
//!   Logfire platform.
//!
//! All environment variables supported by the Rust Opentelemetry SDK are also supported by the
//! Logfire SDK.
//!
//! ## Integrations
//!
//! The following sections describe briefly the interaction which this SDK has with other libraries.
//!
//! ### With `tracing`
//!
//! This SDK is built upon `tracing` (and `tracing-opentelemtry`) for the [`span!`] macro. This means
//! that any code instrumented with `tracing` will automatically be captured by Logfire, and also
//! that [`span!`] produces a `tracing::Span` which is fully compatible with the `tracing` ecosystem.
//!
//! If you are an existing `tracing` user, it is fine to continue to use the `tracing` APIs directly
//! and ignore [`logfire::span!`][span]. The upside of [`span!`] is that it will show the fields
//! directly in the logfire UI.
//!
//! There are many great APIs in `tracing` which we do not yet provide equivalents for, such as the
//! [`#[tracing::instrument]`][`macro@tracing::instrument`] proc macro, so even if using [`logfire::span!`][span]
//! you will likely use `tracing` APIs directly too.
//!
//! ### With `opentelemetry`
//!
//! This SDK is built upon the `opentelemetry` Rust SDK and will configure the global `opentelemetry`
//! state as part of a call to [`logfire::configure()`][configure].
//!
//! All calls to [`logfire::info!`][info] and similar macros are directly forwarded to `opentelemetry`
//! machinery without going through `tracing`, for performance.
//!
//! The metrics helpers exported by this SDK, such as [`logfire::u64_counter()`][u64_counter], are
//! very thin wrappers around the `opentelemetry` SDK.
//!
//! ### With `log`
//!
//! This SDK configures the global `log` state to use an exporter which forwards logs to opentelemetry.
//!
//! All code instrumented with `log` will therefore automatically be captured by Logfire.

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::PanicHookInfo;
use std::sync::{Arc, Once};
use std::{backtrace::Backtrace, env::VarError, str::FromStr, sync::OnceLock, time::Duration};

use bridges::tracing::LogfireTracingLayer;
use config::{AdvancedOptions, BoxedSpanProcessor, ConsoleOptions, SendToLogfire};
use internal::exporters::console::SimpleConsoleSpanExporter;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, BatchSpanProcessor, SimpleSpanProcessor, SpanProcessor,
};
use opentelemetry_sdk::trace::{SdkTracerProvider, Tracer};
use thiserror::Error;
use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

mod bridges;
pub mod config;
pub mod exporters;
mod macros;
mod metrics;
mod ulid_id_generator;

pub use macros::*;
pub use metrics::*;
use ulid_id_generator::UlidIdGenerator;

mod internal;

/// An error which may arise when configuring Logfire.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigureError {
    /// No token was provided to send to logfire.
    #[error(
        "A logfire token is required from either `logfire::configure().with_token()` or the `LOGFIRE_TOKEN` environment variable"
    )]
    TokenRequired,

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

    /// Error parsing the `RUST_LOG` environment variable.
    #[error("Error configuring the OpenTelemetry tracer: {0}")]
    RustLogInvalid(#[from] tracing_subscriber::filter::FromEnvError),

    /// A Rust feature needs to be enabled in the `Cargo.toml`.
    #[error("Rust feature required: `{feature_name}` feature must be enabled for {functionality}")]
    LogfireFeatureRequired {
        /// The feature which is required.
        feature_name: &'static str,
        /// The functionality which was attempted to be used.
        functionality: String,
    },

    /// A configuration value (from environment) was invalid.
    #[error("Invalid configuration value for {parameter}: {value}")]
    InvalidConfigurationValue {
        /// The name of the configuration parameter.
        parameter: &'static str,
        /// The invalid value passed for the parameter.
        value: String,
    },

    /// Any other error.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// Whether to print to the console.
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

/// Main entry point to configure logfire.
///
/// This should be called once at the start of the program.
///
/// See [`LogfireConfigBuilder`] for the full set of configuration options.
///
/// # Example
///
/// ```rust
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let shutdown_handler = logfire::configure()
///         .install_panic_handler()
/// #        .send_to_logfire(logfire::config::SendToLogfire::IfTokenPresent)
///         .finish()?;
///
///     logfire::info!("Hello world");
///
///     shutdown_handler.shutdown()?;
///     Ok(())
/// }
/// ```
#[must_use = "call `.finish()` to complete logfire configuration."]
pub fn configure() -> LogfireConfigBuilder {
    LogfireConfigBuilder {
        send_to_logfire: None,
        token: None,
        console_options: None,
        additional_span_processors: Vec::new(),
        advanced: None,
        install_panic_handler: false,
        default_level_filter: None,
    }
}

/// Builder for logfire configuration, returned from [`logfire::configure()`][configure].
pub struct LogfireConfigBuilder {
    // TODO: support all options supported by the Python SDK
    // local: bool,
    send_to_logfire: Option<SendToLogfire>,
    token: Option<String>,
    // service_name: Option<String>,
    // service_version: Option<String>,
    // environment: Option<String>,
    console_options: Option<ConsoleOptions>,

    // config_dir: Option<PathBuf>,
    // data_dir: Option<Path>,

    // TODO: change to match Python SDK
    additional_span_processors: Vec<BoxedSpanProcessor>,
    // tracer_provider: Option<SdkTracerProvider>,

    // TODO: advanced Python options not yet supported by the Rust SDK
    // metrics: MetricsOptions | Literal[False] | None = None,
    // scrubbing: ScrubbingOptions | Literal[False] | None = None,
    // inspect_arguments: bool | None = None,
    // sampling: SamplingOptions | None = None,
    // code_source: CodeSource | None = None,
    // distributed_tracing: bool | None = None,
    advanced: Option<AdvancedOptions>,

    // Rust specific options
    install_panic_handler: bool,
    default_level_filter: Option<LevelFilter>,
}

impl LogfireConfigBuilder {
    /// Call to install a hook to log panics.
    ///
    /// Any existing panic hook will be preserved and called after the logfire panic hook.
    #[must_use]
    pub fn install_panic_handler(mut self) -> Self {
        self.install_panic_handler = true;
        self
    }

    /// Whether to send data to the Logfire platform.
    ///
    /// Defaults to the value of `LOGFIRE_SEND_TO_LOGFIRE` if set, otherwise `Yes`.
    #[must_use]
    pub fn send_to_logfire<T: Into<SendToLogfire>>(mut self, send_to_logfire: T) -> Self {
        self.send_to_logfire = Some(send_to_logfire.into());
        self
    }

    /// The token to use for the Logfire platform.
    ///
    /// Defaults to the value of `LOGFIRE_TOKEN` if set.
    #[must_use]
    pub fn with_token<T: Into<String>>(mut self, token: T) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Whether to log to the console.
    #[must_use]
    pub fn console_mode(mut self, console_mode: ConsoleMode) -> Self {
        // FIXME: remove this API and make it match Python, see `console_options()` below
        match console_mode {
            ConsoleMode::Fallback => {
                self.console_options = None;
            }
            ConsoleMode::Force => self.console_options = Some(ConsoleOptions::default()),
        }
        self
    }

    /// Set the options for logging to console.
    #[cfg(test)] // FIXME: not all options exposed actually work yet, so not public
    pub fn console_options(mut self, console_options: ConsoleOptions) -> Self {
        self.console_options = Some(console_options);
        self
    }

    /// Override the filter used for traces and logs.
    ///
    /// By default this is set to `LevelFilter::TRACE` if sending to logfire, or `LevelFilter::INFO` if not.
    ///
    /// The `RUST_LOG` environment variable will override this.
    #[must_use]
    pub fn with_default_level_filter(mut self, default_level_filter: LevelFilter) -> Self {
        self.default_level_filter = Some(default_level_filter);
        self
    }

    /// Add an additional span processor to process spans alongside the main logfire exporter.
    ///
    /// To disable the main logfire exporter, set `send_to_logfire` to `false`.
    #[must_use]
    pub fn with_additional_span_processor<T: SpanProcessor + 'static>(
        mut self,
        span_processor: T,
    ) -> Self {
        self.additional_span_processors
            .push(BoxedSpanProcessor::new(Box::new(span_processor)));
        self
    }

    /// Configure [advanced options](crate::config::AdvancedOptions).
    #[must_use]
    pub fn with_advanced_options(mut self, advanced: AdvancedOptions) -> Self {
        self.advanced = Some(advanced);
        self
    }

    /// Finish configuring Logfire.
    ///
    /// Because this configures global state for the opentelemetry SDK, this can typically only ever be called once per program.
    ///
    /// # Errors
    ///
    /// See [`ConfigureError`] for possible errors.
    pub fn finish(self) -> Result<ShutdownHandler, ConfigureError> {
        let LogfireParts {
            tracer,
            subscriber,
            tracer_provider,
            base_url,
            http_headers,
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
                PeriodicReader::builder(exporters::metric_exporter(&base_url, http_headers)?)
                    .build();

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
        self,
        env: Option<&HashMap<String, String>>,
    ) -> Result<LogfireParts, ConfigureError> {
        let mut token = self.token;
        if token.is_none() {
            token = get_optional_env("LOGFIRE_TOKEN", env)?;
        }

        let send_to_logfire = match self.send_to_logfire {
            Some(send_to_logfire) => send_to_logfire,
            None => match get_optional_env("LOGFIRE_SEND_TO_LOGFIRE", env)? {
                Some(value) => value.parse()?,
                None => SendToLogfire::Yes,
            },
        };

        let send_to_logfire = match send_to_logfire {
            SendToLogfire::Yes => true,
            SendToLogfire::IfTokenPresent => token.is_some(),
            SendToLogfire::No => false,
        };

        let advanced_options = self.advanced.unwrap_or_default();

        let mut tracer_provider_builder = SdkTracerProvider::builder();

        if let Some(id_generator) = advanced_options.id_generator {
            tracer_provider_builder = tracer_provider_builder.with_id_generator(id_generator);
        } else {
            tracer_provider_builder =
                tracer_provider_builder.with_id_generator(UlidIdGenerator::new());
        };

        let mut http_headers: Option<HashMap<String, String>> = None;

        if send_to_logfire {
            let Some(token) = token else {
                return Err(ConfigureError::TokenRequired);
            };
            http_headers
                .get_or_insert_default()
                .insert("Authorization".to_string(), format!("Bearer {token}"));

            tracer_provider_builder = tracer_provider_builder.with_span_processor(
                BatchSpanProcessor::builder(exporters::span_exporter(
                    &advanced_options.base_url,
                    http_headers.clone(),
                )?)
                .with_batch_config(
                    BatchConfigBuilder::default()
                        .with_scheduled_delay(Duration::from_millis(500)) // 500 matches Python
                        .build(),
                )
                .build(),
            );
        }

        // TODO make this behaviour closer to Python
        let mut console_options = self.console_options;
        if console_options.is_none() && !send_to_logfire {
            // FIXME: in Python the console and logfire settings are independent, we should not have
            // "fallback" like this.
            console_options = Some(ConsoleOptions::default());
        }

        if let Some(console_options) = console_options {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(
                SimpleSpanProcessor::new(Box::new(SimpleConsoleSpanExporter::new(console_options))),
            );
        }

        for span_processor in self.additional_span_processors {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(span_processor);
        }

        let tracer_provider = tracer_provider_builder.build();

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
            base_url: advanced_options.base_url,
            http_headers,
            send_to_logfire,
        })
    }
}

/// A handler to shutdown the Logfire configuration.
///
/// Calling `.shutdown()` will flush the logfire exporters and make further
/// logfire calls into no-ops.
#[derive(Debug, Clone)]
#[must_use = "this should be kept alive until logging should be stopped"]
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
    base_url: String,
    http_headers: Option<HashMap<String, String>>,
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
        trace::{IdGenerator, InMemorySpanExporterBuilder, SpanData, SpanExporter},
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

        let config = crate::configure()
            .send_to_logfire(false)
            .with_additional_span_processor(SimpleSpanProcessor::new(Box::new(
                DeterministicExporter::new(exporter.clone()),
            )))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .with_advanced_options(
                AdvancedOptions::default().with_id_generator(DeterministicIdGenerator::new()),
            );

        let guard = set_local_logfire(config).unwrap();

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
                            829,
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
                            830,
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
                            830,
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
                            831,
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
                            833,
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
                            834,
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
                                "src/lib.rs:835:17",
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
                            589,
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
                            829,
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
            (vec![], None, Err(ConfigureError::TokenRequired)),
            (vec![("LOGFIRE_TOKEN", "a")], None, Ok(true)),
            (vec![("LOGFIRE_SEND_TO_LOGFIRE", "no")], None, Ok(false)),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "yes")],
                None,
                Err(ConfigureError::TokenRequired),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "yes"), ("LOGFIRE_TOKEN", "a")],
                None,
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "if-token-present")],
                None,
                Ok(false),
            ),
            (
                vec![
                    ("LOGFIRE_SEND_TO_LOGFIRE", "if-token-present"),
                    ("LOGFIRE_TOKEN", "a"),
                ],
                None,
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no"), ("LOGFIRE_TOKEN", "a")],
                Some(SendToLogfire::Yes),
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no"), ("LOGFIRE_TOKEN", "a")],
                Some(SendToLogfire::IfTokenPresent),
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no")],
                Some(SendToLogfire::IfTokenPresent),
                Ok(false),
            ),
        ] {
            let env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();

            let mut config = crate::configure();
            if let Some(value) = setting {
                config = config.send_to_logfire(value);
            }

            let result = config
                .build_parts(Some(&env))
                .map(|parts| parts.send_to_logfire);

            match (expected, result) {
                (Ok(exp), Ok(actual)) => assert_eq!(exp, actual),
                // compare strings because ConfigureError doesn't implement PartialEq
                (Err(exp), Err(actual)) => assert_eq!(exp.to_string(), actual.to_string()),
                (expected, result) => panic!("expected {expected:?}, got {result:?}"),
            }
        }
    }
}
