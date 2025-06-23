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
use std::{backtrace::Backtrace, env::VarError, sync::OnceLock, time::Duration};

use config::get_base_url_from_token;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, SpanProcessor};
use opentelemetry_sdk::trace::{SdkTracerProvider, Tracer};
use thiserror::Error;
use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing::subscriber::DefaultGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;

use crate::bridges::tracing::LogfireTracingLayer;
use crate::config::{
    AdvancedOptions, BoxedSpanProcessor, ConsoleOptions, MetricsOptions, SendToLogfire,
};
use crate::internal::exporters::console::{ConsoleWriter, SimpleConsoleSpanProcessor};

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

    /// Error configuring the OpenTelemetry tracer.
    #[error("Error configuring the OpenTelemetry tracer: {0}")]
    Trace(#[from] opentelemetry_sdk::trace::TraceError),

    /// OpenTelemetry exporter failed to build
    #[error("Error building the OpenTelemetry exporter: {0}")]
    ExporterBuildError(#[from] opentelemetry_otlp::ExporterBuildError),

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

#[expect(deprecated)]
mod deprecated {
    use std::str::FromStr;

    /// Whether to print to the console.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
    #[deprecated(since = "0.4.0", note = "use `ConsoleOptions` instead")]
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
}

#[expect(deprecated)]
use deprecated::ConsoleMode;

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
        local: false,
        send_to_logfire: None,
        token: None,
        console_options: Some(ConsoleOptions::default()),
        #[expect(deprecated)]
        console_mode: ConsoleMode::Force,
        additional_span_processors: Vec::new(),
        advanced: None,
        metrics: Some(MetricsOptions::default()),
        enable_metrics: true,
        install_panic_handler: false,
        default_level_filter: None,
    }
}

/// Builder for logfire configuration, returned from [`logfire::configure()`][configure].
pub struct LogfireConfigBuilder {
    local: bool,
    send_to_logfire: Option<SendToLogfire>,
    token: Option<String>,
    // service_name: Option<String>,
    // service_version: Option<String>,
    // environment: Option<String>,
    console_options: Option<ConsoleOptions>,
    /// Deprecated setting, `Force` implies use `console_options`, `Fallback` will filter
    /// them out if `send_to_logfire` is false.
    #[expect(deprecated)]
    console_mode: ConsoleMode,

    // config_dir: Option<PathBuf>,
    // data_dir: Option<Path>,

    // TODO: change to match Python SDK
    additional_span_processors: Vec<BoxedSpanProcessor>,
    // tracer_provider: Option<SdkTracerProvider>,

    // TODO: advanced Python options not yet supported by the Rust SDK
    // scrubbing: ScrubbingOptions | Literal[False] | None = None,
    // inspect_arguments: bool | None = None,
    // sampling: SamplingOptions | None = None,
    // code_source: CodeSource | None = None,
    // distributed_tracing: bool | None = None,
    advanced: Option<AdvancedOptions>,
    metrics: Option<MetricsOptions>,
    enable_metrics: bool,

    // Rust specific options
    install_panic_handler: bool,
    default_level_filter: Option<LevelFilter>,
}

impl LogfireConfigBuilder {
    /// Call to configure Logfire for local use only.
    ///
    /// This prevents the configured `Logfire` from setting global `tracing`, `log` and `opentelemetry` state.
    #[doc(hidden)]
    #[must_use]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }

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
    #[deprecated(since = "0.4.0", note = "use `with_console()` instead")]
    #[expect(deprecated)]
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

    /// Sets console options. Set to `None` to disable console logging.
    ///
    /// If not set, will use `ConsoleOptions::default()`.
    #[must_use]
    pub fn with_console(mut self, console_options: Option<ConsoleOptions>) -> Self {
        self.console_options = console_options;
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

    /// Configure [metrics options](crate::config::MetricsOptions).
    ///
    /// Set to `None` to disable metrics.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Option<MetricsOptions>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Configure [metrics options](crate::config::MetricsOptions).
    #[must_use]
    #[deprecated(since = "0.4.0", note = "use `with_metrics` instead")]
    pub fn with_metrics_options(mut self, metrics: MetricsOptions) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Whether to enable metrics.
    #[must_use]
    #[deprecated(since = "0.4.0", note = "use `with_metrics(None)` to disable metrics")]
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
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
            local,
            tracer,
            subscriber,
            tracer_provider,
            meter_provider,
            ..
        } = self.build_parts(None)?;

        if !local {
            tracing::subscriber::set_global_default(subscriber.clone())?;
            let logger = bridges::log::LogfireLogger::init(tracer.inner.clone());
            log::set_logger(logger)?;
            log::set_max_level(logger.max_level());

            GLOBAL_TRACER
                .set(tracer.clone())
                .map_err(|_| ConfigureError::AlreadyConfigured)?;

            let propagator = opentelemetry::propagation::TextMapCompositePropagator::new(vec![
                Box::new(opentelemetry_sdk::propagation::TraceContextPropagator::new()),
                Box::new(opentelemetry_sdk::propagation::BaggagePropagator::new()),
            ]);
            opentelemetry::global::set_text_map_propagator(propagator);

            opentelemetry::global::set_meter_provider(meter_provider.clone());
        }

        Ok(ShutdownHandler {
            tracer_provider,
            tracer,
            subscriber,
            meter_provider,
        })
    }

    #[expect(clippy::too_many_lines)]
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
        }

        if let Some(resource) = advanced_options.resource.clone() {
            tracer_provider_builder = tracer_provider_builder.with_resource(resource);
        }

        let mut http_headers: Option<HashMap<String, String>> = None;

        let logfire_base_url = if send_to_logfire {
            let Some(token) = token else {
                return Err(ConfigureError::TokenRequired);
            };

            http_headers
                .get_or_insert_default()
                .insert("Authorization".to_string(), format!("Bearer {token}"));

            Some(
                advanced_options
                    .base_url
                    .as_deref()
                    .unwrap_or_else(|| get_base_url_from_token(&token)),
            )
        } else {
            None
        };

        if let Some(logfire_base_url) = logfire_base_url {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(
                BatchSpanProcessor::builder(exporters::span_exporter(
                    logfire_base_url,
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

        let console_writer = self
            .console_options
            // NB deprecated behaviour: if set to fallback and sending to logfire, disable console
            .filter(
                #[expect(deprecated)]
                |_| !(self.console_mode == ConsoleMode::Fallback && send_to_logfire),
            )
            .map(ConsoleWriter::new)
            .map(Arc::new);

        if let Some(console_writer) = console_writer.clone() {
            tracer_provider_builder = tracer_provider_builder
                .with_span_processor(SimpleConsoleSpanProcessor::new(console_writer));
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

        let tracer = LogfireTracer {
            inner: tracer,
            handle_panics: self.install_panic_handler,
        };

        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(LogfireTracingLayer::new(tracer.clone()));

        let mut meter_provider_builder = SdkMeterProvider::builder();

        if let Some(logfire_base_url) = logfire_base_url {
            if self.enable_metrics {
                let metric_reader = PeriodicReader::builder(exporters::metric_exporter(
                    logfire_base_url,
                    http_headers,
                )?)
                .build();

                meter_provider_builder = meter_provider_builder.with_reader(metric_reader);
            }
        }

        if let Some(metrics) = self.metrics.filter(|_| self.enable_metrics) {
            for reader in metrics.additional_readers {
                meter_provider_builder = meter_provider_builder.with_reader(reader);
            }
        }

        if let Some(resource) = advanced_options.resource {
            meter_provider_builder = meter_provider_builder.with_resource(resource);
        }

        let meter_provider = meter_provider_builder.build();

        if self.install_panic_handler {
            install_panic_handler();
        }

        Ok(LogfireParts {
            local: self.local,
            tracer,
            subscriber: Arc::new(subscriber),
            tracer_provider,
            meter_provider,
            #[cfg(test)]
            send_to_logfire,
        })
    }
}

/// A handler to shutdown the Logfire configuration.
///
/// Calling `.shutdown()` will flush the logfire exporters and make further
/// logfire calls into no-ops.
#[derive(Clone)]
#[must_use = "this should be kept alive until logging should be stopped"]
pub struct ShutdownHandler {
    tracer_provider: SdkTracerProvider,
    tracer: LogfireTracer,
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    meter_provider: SdkMeterProvider,
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
        self.meter_provider
            .shutdown()
            .map_err(|e| ConfigureError::Other(e.into()))?;
        Ok(())
    }

    /// Get a tracing layer which can be used to embed this `Logfire` instance into a `tracing_subscriber::Registry`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tracing_subscriber::{Registry, layer::SubscriberExt};
    ///
    /// let shutdown_handler = logfire::configure()
    ///    .local()  // use local mode to avoid setting global state
    ///    .finish()
    ///    .expect("Failed to configure logfire");
    ///
    /// let subscriber = tracing_subscriber::registry()
    ///    .with(shutdown_handler.tracing_layer());
    ///
    /// tracing::subscriber::set_global_default(subscriber)
    ///    .expect("Failed to set global subscriber");
    ///
    /// logfire::info!("Hello world");
    ///
    /// shutdown_handler.shutdown().expect("Failed to shutdown logfire");
    /// ```
    #[must_use]
    pub fn tracing_layer<S>(&self) -> LogfireTracingLayer<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        LogfireTracingLayer::new(self.tracer.clone())
    }
}

struct LogfireParts {
    local: bool,
    tracer: LogfireTracer,
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    #[cfg(test)]
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

        // FIXME: code.lineno and code.filepath should probably be set here to the panic location
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

#[derive(Clone)]
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
#[doc(hidden)]
pub struct LocalLogfireGuard {
    prior: Option<LogfireTracer>,
    #[expect(dead_code, reason = "tracing RAII guard")]
    tracing_guard: DefaultGuard,
    /// Shutdown handler
    shutdown_handler: ShutdownHandler,
}

impl LocalLogfireGuard {
    /// Get the current tracer.
    #[must_use]
    pub fn subscriber(&self) -> Arc<dyn Subscriber + Send + Sync> {
        self.shutdown_handler.subscriber.clone()
    }

    /// Ge the current meter provider
    #[must_use]
    pub fn meter_provider(&self) -> &SdkMeterProvider {
        &self.shutdown_handler.meter_provider
    }
}

impl Drop for LocalLogfireGuard {
    fn drop(&mut self) {
        // FIXME: if drop order is not consistent with creation order, does this create strange
        // state?
        LOCAL_TRACER.with_borrow_mut(|local_logfire| {
            *local_logfire = self.prior.take();
        });
    }
}

#[doc(hidden)] // used in tests
#[must_use]
pub fn set_local_logfire(shutdown_handler: ShutdownHandler) -> LocalLogfireGuard {
    let prior = LOCAL_TRACER
        .with_borrow_mut(|local_logfire| local_logfire.replace(shutdown_handler.tracer.clone()));

    let tracing_guard = tracing::subscriber::set_default(shutdown_handler.subscriber.clone());

    // TODO: logs??
    // TODO: metrics??

    LocalLogfireGuard {
        prior,
        tracing_guard,
        shutdown_handler,
    }
}

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests {
    use crate::{ConfigureError, config::SendToLogfire};

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
