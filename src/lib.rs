#![doc(html_favicon_url = "https://pydantic.dev/favicon/favicon.ico")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/pydantic/logfire-rust/refs/heads/main/assets/logo-white.svg"
)]

//! # Rust SDK for Pydantic Logfire
//!
//! From the team behind Pydantic Validation, **Pydantic Logfire** is an observability platform built on the same belief as our open source library — that the most powerful tools can be easy to use.
//!
//! The most important API is [`logfire::configure()`][configure], which is used to set up
//! integrations with `tracing` and `log`, as well as exporters for `opentelemetry`. Code
//! instrumented using `tracing` and `log` will *just work* once this configuration is in place.
//!
//! This SDK also offers opinionated functions to instrument code following Logfire's design principles:
//!   - **traces**: [`logfire::span!()`][span] to create a span with structured data.
//!   - **logs**: [`logfire::info!()`][info], [`logfire::debug!()`][debug] and similar macros to log messages
//!     with structured data.
//!   - **metrics**: [`logfire::u64_counter()`][u64_counter] and similar functions.
//!
//! See the [Logfire documentation](https://logfire.pydantic.dev/docs/) for more information about
//! Logfire in general, and the [Logfire GitHub repository](https://github.com/pydantic/logfire) for
//! the source of that documentation, the Python SDK and an issue tracker for general questions
//! about Logfire.
//!
//! # Usage
//!
//! The setup for this SDK is a quick three-step process:
//!  1. Add the `logfire` crate to your `Cargo.toml`.
//!  2. Call [`logfire::configure()`][configure] at the start of your program to set up the SDK.
//!  3. Run your application with the [`LOGFIRE_TOKEN`] environment variable set to connect it to the Logfire platform.
//!
//! This process is demonstrated below. The [usage guide][usage] contains more detailed information about how to use this
//! SDK to its full potential.
//!
//! ## Getting Started
//!
//! To use `logfire` in your Rust project, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
#![doc = concat!("logfire = \"", env!("CARGO_PKG_VERSION"), "\"\n")]
//! ```
//!
//! Then, in your Rust code, add a call to [`logfire::configure()`][configure] at the beginning of your program:
//!
//! ```rust
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let shutdown_handler = logfire::configure()
//!         .install_panic_handler()
//! #        .send_to_logfire(logfire::config::SendToLogfire::IfTokenPresent)
//!         .finish()?;
//!
//!     logfire::info!("Hello world");
//!
//!     shutdown_handler.shutdown()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration
//!
//! After adding basic setup as per above, the most two important environment variables are:
//! - [`LOGFIRE_TOKEN`] (required) - the token to send data to the Logfire platform
//! - [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/#filtering-results) (optional) - the level of verbosity to send to the Logfire platform. By default
//!   data is captured at `TRACE` level so that all data is available for you to analyze in the
//!   Logfire platform. This format should match the format used by the [`env_logger`](https://docs.rs/env_logger/) crate.
//!
//! All environment variables supported by the Rust Opentelemetry SDK are also supported by the
//! Logfire SDK.
//!
//! # Examples
//!
//! See [examples][usage::examples] subchapter of this documentation.
//!
//! [`LOGFIRE_TOKEN`]: https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens/

use std::{
    backtrace::Backtrace,
    borrow::Cow,
    collections::HashMap,
    env::VarError,
    panic::PanicHookInfo,
    sync::{Arc, Once},
    time::Duration,
};

use config::get_base_url_from_token;
use opentelemetry::{
    logs::{LoggerProvider as _, Severity},
    trace::TracerProvider,
};
use opentelemetry_sdk::{
    logs::{BatchLogProcessor, SdkLoggerProvider},
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::{BatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider, SpanProcessor},
};
use thiserror::Error;
use tracing::{Subscriber, level_filters::LevelFilter, subscriber::DefaultGuard};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan};

use crate::{
    __macros_impl::LogfireValue,
    config::{AdvancedOptions, BoxedSpanProcessor, ConsoleOptions, MetricsOptions, SendToLogfire},
    internal::{
        exporters::console::{ConsoleWriter, create_console_processors},
        logfire_tracer::{GLOBAL_TRACER, LOCAL_TRACER, LogfireTracer},
    },
    ulid_id_generator::UlidIdGenerator,
};

mod macros;

#[cfg(any(docsrs, doctest))]
pub mod usage;

mod bridges;
pub mod config;
pub mod exporters;
mod metrics;
mod ulid_id_generator;

pub use macros::*;
pub use metrics::*;

pub use crate::bridges::tracing::LogfireTracingLayer;

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
            logger_provider,
            enable_tracing_metrics,
            ..
        } = self.build_parts(None)?;

        if !local {
            tracing::subscriber::set_global_default(subscriber.clone())?;
            let logger = bridges::log::LogfireLogger::init(tracer.clone());
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
            logger_provider,
            enable_tracing_metrics,
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
        let mut logger_provider_builder = SdkLoggerProvider::builder();

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

        let console_processors = self
            .console_options
            .map(|o| create_console_processors(Arc::new(ConsoleWriter::new(o))));

        if let Some((span_processor, log_processor)) = console_processors {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(span_processor);
            logger_provider_builder = logger_provider_builder.with_log_processor(log_processor);
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

        let subscriber = tracing_subscriber::registry().with(filter);

        let mut meter_provider_builder = SdkMeterProvider::builder();

        if let Some(logfire_base_url) = logfire_base_url {
            if self.enable_metrics {
                let metric_reader = PeriodicReader::builder(exporters::metric_exporter(
                    logfire_base_url,
                    http_headers.clone(),
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

        if let Some(resource) = advanced_options.resource.clone() {
            meter_provider_builder = meter_provider_builder.with_resource(resource);
        }

        let meter_provider = meter_provider_builder.build();

        if let Some(logfire_base_url) = logfire_base_url {
            logger_provider_builder = logger_provider_builder.with_log_processor(
                BatchLogProcessor::builder(exporters::log_exporter(
                    logfire_base_url,
                    http_headers.clone(),
                )?)
                .build(),
            );
        }

        for log_processor in advanced_options.log_record_processors {
            logger_provider_builder = logger_provider_builder.with_log_processor(log_processor);
        }

        if let Some(resource) = advanced_options.resource {
            logger_provider_builder = logger_provider_builder.with_resource(resource);
        }

        let logger_provider = logger_provider_builder.build();

        let logger = Arc::new(logger_provider.logger("logfire"));

        let mut filter_builder = env_filter::Builder::new();
        if let Ok(filter) = std::env::var("RUST_LOG") {
            filter_builder.parse(&filter);
        } else {
            filter_builder.parse(&default_level_filter.to_string());
        }

        let tracer = LogfireTracer {
            inner: tracer,
            meter_provider: meter_provider.clone(),
            logger,
            handle_panics: self.install_panic_handler,
            filter: Arc::new(filter_builder.build()),
        };

        let subscriber = subscriber.with(LogfireTracingLayer::new(
            tracer.clone(),
            advanced_options.enable_tracing_metrics,
        ));

        if self.install_panic_handler {
            install_panic_handler();
        }

        Ok(LogfireParts {
            local: self.local,
            tracer,
            subscriber: Arc::new(subscriber),
            tracer_provider,
            meter_provider,
            logger_provider,
            enable_tracing_metrics: advanced_options.enable_tracing_metrics,
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
    logger_provider: SdkLoggerProvider,
    enable_tracing_metrics: bool,
}

#[allow(clippy::print_stderr)]
impl Drop for ShutdownHandler {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("failed to shutdown logfire cleanly: {error:#?}");
        }
    }
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
        self.logger_provider
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
        LogfireTracingLayer::new(self.tracer.clone(), self.enable_tracing_metrics)
    }
}

struct LogfireParts {
    local: bool,
    tracer: LogfireTracer,
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
    enable_tracing_metrics: bool,
    #[cfg(test)]
    send_to_logfire: bool,
}

/// Install `handler` as part of a chain of panic handlers.
fn install_panic_handler() {
    fn panic_hook(info: &PanicHookInfo) {
        LogfireTracer::try_with(|tracer| {
            if !tracer.handle_panics {
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

            let location = info.location();
            tracer.export_log(
                "panic",
                &tracing::Span::current().context(),
                format!("panic: {message}"),
                Severity::Error,
                crate::__json_schema!(backtrace),
                location.map(|l| Cow::Owned(l.file().to_string())),
                location.map(std::panic::Location::line),
                None,
                [LogfireValue::new(
                    "backtrace",
                    Some(Backtrace::capture().to_string().into()),
                )],
            );
        });
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
