//! Configuration options for the Logfire SDK.
//!
//! See [`LogfireConfigBuilder`] for documentation of all these options.

use std::{
    collections::HashMap,
    convert::Infallible,
    fmt::Display,
    marker::PhantomData,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

use opentelemetry_sdk::{
    logs::LogProcessor,
    metrics::reader::MetricReader,
    trace::{IdGenerator, ShouldSample, SpanProcessor, TracerProviderBuilder},
};
use regex::Regex;
use tracing::{Level, level_filters::LevelFilter};

use crate::{ConfigureError, internal::env::get_optional_env, logfire::Logfire};

/// Builder for logfire configuration, returned from [`logfire::configure()`][crate::configure].
#[must_use = "call `.finish()` to complete logfire configuration."]
pub struct LogfireConfigBuilder {
    pub(crate) local: bool,
    pub(crate) send_to_logfire: Option<SendToLogfire>,
    pub(crate) token: Option<String>,
    pub(crate) service_name: Option<String>,
    pub(crate) service_version: Option<String>,
    pub(crate) environment: Option<String>,
    pub(crate) console_options: Option<ConsoleOptions>,

    // config_dir: Option<PathBuf>,
    pub(crate) data_dir: Option<PathBuf>,

    // TODO: change to match Python SDK
    pub(crate) additional_span_processors: Vec<BoxedSpanProcessor>,
    // tracer_provider: Option<SdkTracerProvider>,

    // TODO: advanced Python options not yet supported by the Rust SDK
    // scrubbing: ScrubbingOptions | Literal[False] | None = None,
    // inspect_arguments: bool | None = None,
    // sampling: SamplingOptions | None = None,
    // code_source: CodeSource | None = None,
    // distributed_tracing: bool | None = None,
    pub(crate) advanced: Option<AdvancedOptions>,
    pub(crate) metrics: Option<MetricsOptions>,

    // Rust specific options
    pub(crate) install_panic_handler: bool,
    pub(crate) default_level_filter: Option<LevelFilter>,
}

impl Default for LogfireConfigBuilder {
    fn default() -> Self {
        Self {
            local: false,
            send_to_logfire: None,
            token: None,
            service_name: None,
            service_version: None,
            environment: None,
            console_options: None,
            data_dir: None,
            additional_span_processors: Vec::new(),
            advanced: None,
            metrics: None,
            install_panic_handler: true,
            default_level_filter: None,
        }
    }
}

impl LogfireConfigBuilder {
    /// Call to configure Logfire for local use only.
    ///
    /// This prevents the configured `Logfire` from setting global `tracing`, `log` and `opentelemetry` state.
    #[doc(hidden)] // FIXME make `LocalLogfireGuard` properly public
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }

    /// Whether to install a hook to
    ///
    /// Any existing panic hook will be preserved and called after the logfire panic hook.
    pub fn with_install_panic_handler(mut self, install: bool) -> Self {
        self.install_panic_handler = install;
        self
    }

    /// Deprecated form of [`with_install_panic_handler`][Self::with_install_panic_handler].
    #[deprecated(since = "0.8.0", note = "noop; now installed by default")]
    pub fn install_panic_handler(self) -> Self {
        self
    }

    /// Whether to send data to the Logfire platform.
    ///
    /// Defaults to the value of `LOGFIRE_SEND_TO_LOGFIRE` if set, otherwise `Yes`.
    pub fn send_to_logfire<T: Into<SendToLogfire>>(mut self, send_to_logfire: T) -> Self {
        self.send_to_logfire = Some(send_to_logfire.into());
        self
    }

    /// The token to use for the Logfire platform.
    ///
    /// Defaults to the value of `LOGFIRE_TOKEN` if set.
    pub fn with_token<T: Into<String>>(mut self, token: T) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set the [service name] for the application.
    ///
    /// [service name]: https://opentelemetry.io/docs/specs/semconv/registry/attributes/service/#service-name
    pub fn with_service_name<T: Into<String>>(mut self, service_name: T) -> Self {
        self.service_name = Some(service_name.into());
        self
    }

    /// Set the [service version] for the application.
    ///
    /// [service version]: https://opentelemetry.io/docs/specs/semconv/registry/attributes/service/#service-version
    pub fn with_service_version<T: Into<String>>(mut self, service_version: T) -> Self {
        self.service_version = Some(service_version.into());
        self
    }

    /// Set the [deployment environment] for the application.
    ///
    /// [deployment environment]: https://opentelemetry.io/docs/specs/semconv/registry/attributes/deployment/#deployment-environment-name
    pub fn with_environment<T: Into<String>>(mut self, environment: T) -> Self {
        self.environment = Some(environment.into());
        self
    }

    /// Sets console options. Set to `None` to disable console logging.
    ///
    /// If not set, will use `ConsoleOptions::default()`.
    pub fn with_console(mut self, console_options: Option<ConsoleOptions>) -> Self {
        self.console_options = console_options;
        self
    }

    /// Sets the directory where credentials are stored. If unset uses the `LOGFIRE_CREDENTIALS_DIR` environment
    /// variable, otherwise defaults to `'.logfire'`.
    pub fn with_data_dir<T: Into<PathBuf>>(mut self, data_dir: T) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    /// Override the filter used for traces and logs.
    ///
    /// By default this is set to `LevelFilter::TRACE` if sending to logfire, or `LevelFilter::INFO` if not.
    ///
    /// The `RUST_LOG` environment variable will override this.
    pub fn with_default_level_filter(mut self, default_level_filter: LevelFilter) -> Self {
        self.default_level_filter = Some(default_level_filter);
        self
    }

    /// Add an additional span processor to process spans alongside the main logfire exporter.
    ///
    /// To disable the main logfire exporter, set `send_to_logfire` to `false`.
    pub fn with_additional_span_processor<T: SpanProcessor + 'static>(
        mut self,
        span_processor: T,
    ) -> Self {
        self.additional_span_processors
            .push(BoxedSpanProcessor::new(Box::new(span_processor)));
        self
    }

    /// Configure [advanced options](crate::config::AdvancedOptions).
    pub fn with_advanced_options(mut self, advanced: AdvancedOptions) -> Self {
        self.advanced = Some(advanced);
        self
    }

    /// Configure [metrics options](crate::config::MetricsOptions).
    ///
    /// Set to `None` to disable metrics.
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
    pub fn finish(self) -> Result<Logfire, ConfigureError> {
        Logfire::from_config_builder(self)
    }
}

/// Whether to send logs to Logfire.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendToLogfire {
    /// Send logs to Logfire.
    #[default]
    Yes,
    /// Do not send logs to Logfire (potentially just print them).
    No,
    /// Send logs to Logfire only if a token is present.
    IfTokenPresent,
}

impl Display for SendToLogfire {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendToLogfire::Yes => write!(f, "yes"),
            SendToLogfire::No => write!(f, "no"),
            SendToLogfire::IfTokenPresent => write!(f, "if-token-present"),
        }
    }
}

impl FromStr for SendToLogfire {
    type Err = ConfigureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yes" => Ok(SendToLogfire::Yes),
            "no" => Ok(SendToLogfire::No),
            "if-token-present" => Ok(SendToLogfire::IfTokenPresent),
            _ => Err(ConfigureError::InvalidConfigurationValue {
                parameter: "LOGFIRE_SEND_TO_LOGFIRE",
                value: s.to_owned(),
            }),
        }
    }
}

impl From<bool> for SendToLogfire {
    fn from(b: bool) -> Self {
        if b {
            SendToLogfire::Yes
        } else {
            SendToLogfire::No
        }
    }
}

/// Options for controlling console output.
#[derive(Debug, Clone)]
pub struct ConsoleOptions {
    /// Whether to show colors in the console.
    pub(crate) colors: ConsoleColors,
    /// Where to send output
    pub(crate) target: Target,
    /// Whether to include timestamps in the console output.
    #[deprecated(
        since = "0.9.0",
        note = "field access will be removed; use builder methods"
    )]
    pub include_timestamps: bool,
    /// The minimum log level to show in the console.
    #[deprecated(
        since = "0.9.0",
        note = "field access will be removed; use builder methods"
    )]
    pub min_log_level: Level,
    // TODO: support the below configuration options (inherited from Python SDK)
    // /// How spans are shown in the console.
    // span_style: SpanStyle,
    // /// Whether to include tags in the console output.
    // include_tags: bool,
    // /// Whether to show verbose output.
    // ///
    // /// It includes the filename, log level, and line number.
    // verbose: bool,
    // /// Whether to print the URL of the Logfire project after initialization.
    // show_project_link: bool,
}

impl Default for ConsoleOptions {
    fn default() -> Self {
        ConsoleOptions {
            colors: ConsoleColors::default(),
            // span_style: SpanStyle::default(),
            // include_tags: true,
            // verbose: false,
            #[expect(deprecated)]
            min_log_level: Level::INFO,
            // show_project_link: true,
            target: Target::default(),
            #[expect(deprecated)]
            include_timestamps: true,
        }
    }
}

impl ConsoleOptions {
    /// Set the target for console output.
    #[must_use]
    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }
}

impl ConsoleOptions {
    /// Control whether to show colors in the console.
    #[must_use]
    pub fn with_colors(mut self, colors: ConsoleColors) -> Self {
        self.colors = colors;
        self
    }

    /// Set whether to include timestamps in the console output.
    #[must_use]
    #[expect(deprecated, reason = "this builder method replaces field access")]
    pub fn with_include_timestamps(mut self, include: bool) -> Self {
        self.include_timestamps = include;
        self
    }

    /// Set the minimum log level to show in the console.
    #[must_use]
    #[expect(deprecated, reason = "this builder method replaces field access")]
    pub fn with_min_log_level(mut self, min_log_level: Level) -> Self {
        self.min_log_level = min_log_level;
        self
    }
}

/// Whether to show colors in the console.
#[derive(Default, Debug, Clone, Copy)]
pub enum ConsoleColors {
    /// Decide based on the terminal.
    #[default]
    Auto,
    /// Always show colors.
    Always,
    /// Never show colors.
    Never,
}

/// Style for rendering spans in the console.
#[derive(Default, Debug, Clone, Copy)]
pub enum SpanStyle {
    /// Show spans in a simple format.
    Simple,
    /// Show spans in a detailed format.
    Indented,
    /// Show parent span ids when printing spans.
    #[default]
    ShowParents,
}

/// Console target, either `stdout`, `stderr` or a custom pipe.
#[derive(Default, Clone)]
pub enum Target {
    /// Console output will be sent to standard output.
    Stdout,
    /// Console output will be sent to standard error.
    #[default]
    Stderr,
    /// Console output will be sent to a custom pipe.
    ///
    /// The outer Arc is useful for testing, (i.e. allows inspecting the output
    /// from a different source.)
    ///
    /// The use of a `Mutex` might not be great for performance due to repeated
    /// locking, however; performance-sensitive use cases might want to just use
    /// stderr (which does lock but is probably better optimized) or not use
    /// console output at all.
    Pipe(Arc<Mutex<dyn std::io::Write + Send + 'static>>),
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Stdout => write!(f, "stdout"),
            Target::Stderr => write!(f, "stderr"),
            Target::Pipe(_) => write!(f, "pipe"),
        }
    }
}

/// Options used for fine-grained control over the logfire SDK.
#[derive(Default)]
pub struct AdvancedOptions {
    pub(crate) base_url: Option<String>,
    pub(crate) id_generator: Option<BoxedIdGenerator>,
    pub(crate) resources: Vec<opentelemetry_sdk::Resource>,
    pub(crate) log_record_processors: Vec<BoxedLogProcessor>,
    pub(crate) enable_tracing_metrics: bool,
    pub(crate) trace_provider: TracerProviderBuilder,
    //
    //
    // TODO: arguments below supported by Python

    // /// Generator for nanosecond start and end timestamps of spans.
    // pub ns_timestamp_generator: Option,
}

impl AdvancedOptions {
    /// Set the base URL for the Logfire API.
    #[must_use]
    pub fn with_base_url<T: AsRef<str>>(mut self, base_url: T) -> Self {
        self.base_url = Some(base_url.as_ref().into());
        self
    }

    /// Set the ID generator for trace and span IDs.
    #[must_use]
    pub fn with_id_generator<T: IdGenerator + Send + Sync + 'static>(
        mut self,
        generator: T,
    ) -> Self {
        self.id_generator = Some(BoxedIdGenerator::new(Box::new(generator)));
        self
    }

    /// Add a [`Resource`](opentelemetry_sdk::Resource) to the tracer, meter,
    /// and logging providers constructed by logfire.
    #[must_use]
    pub fn with_resource(mut self, resource: opentelemetry_sdk::Resource) -> Self {
        self.resources.push(resource);
        self
    }

    /// Add a log processor to the list of log processors. This is experimental and may be removed.
    #[must_use]
    pub fn with_log_processor<T: LogProcessor + Send + Sync + 'static>(
        mut self,
        processor: T,
    ) -> Self {
        self.log_record_processors
            .push(BoxedLogProcessor::new(Box::new(processor)));
        self
    }

    /// Add a custom sampler implementation
    #[must_use]
    pub fn with_sampler<T: ShouldSample + 'static>(mut self, sampler: T) -> Self {
        self.trace_provider = self.trace_provider.with_sampler(sampler);
        self
    }

    /// Support capturing tracing events as metrics as per [`tracing_opentelemetry::MetricsLayer`](https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/struct.MetricsLayer.html).
    #[must_use]
    pub fn with_tracing_metrics(mut self, enable: bool) -> Self {
        self.enable_tracing_metrics = enable;
        self
    }
}

struct RegionData {
    base_url: &'static str,
    #[expect(dead_code)] // not used for the moment
    gcp_region: &'static str,
}

const US_REGION: RegionData = RegionData {
    base_url: "https://logfire-us.pydantic.dev",
    gcp_region: "us-east4",
};

const EU_REGION: RegionData = RegionData {
    base_url: "https://logfire-eu.pydantic.dev",
    gcp_region: "europe-west4",
};

/// Get the base API URL from the token's region.
pub(crate) fn get_base_url_from_token(token: &str) -> &'static str {
    let pydantic_logfire_token_pattern = Regex::new(
        r"^(?P<safe_part>pylf_v(?P<version>[0-9]+)_(?P<region>[a-z]+)_)(?P<token>[a-zA-Z0-9]+)$",
    )
    .expect("token regex is known to be valid");

    #[expect(clippy::wildcard_in_or_patterns, reason = "being explicit about us")]
    match pydantic_logfire_token_pattern
        .captures(token)
        .and_then(|captures| captures.name("region"))
        .map(|region| region.as_str())
    {
        Some("eu") => EU_REGION.base_url,
        // fallback to US region if the token / region is not recognized
        Some("us") | _ => US_REGION.base_url,
    }
}

/// Configuration of metrics.
///
/// This only has one option for now, but it's a place to add more related options in the future.
#[derive(Default)]
pub struct MetricsOptions {
    /// Sequence of metric readers to be used in addition to the default which exports metrics to Logfire's API.
    pub(crate) additional_readers: Vec<BoxedMetricReader>,
}

impl MetricsOptions {
    /// Add a metric reader to the list of additional readers.
    #[must_use]
    pub fn with_additional_reader<T: MetricReader>(mut self, reader: T) -> Self {
        self.additional_readers
            .push(BoxedMetricReader::new(Box::new(reader)));
        self
    }
}

/// Wrapper around a `SpanProcessor` to use in `additional_span_processors`.
#[derive(Debug)]
pub(crate) struct BoxedSpanProcessor(Box<dyn SpanProcessor>);

impl BoxedSpanProcessor {
    pub fn new(processor: Box<dyn SpanProcessor + Send + Sync>) -> Self {
        BoxedSpanProcessor(processor)
    }
}

impl SpanProcessor for BoxedSpanProcessor {
    fn on_start(&self, span: &mut opentelemetry_sdk::trace::Span, cx: &opentelemetry::Context) {
        self.0.on_start(span, cx);
    }

    fn on_end(&self, span: opentelemetry_sdk::trace::SpanData) {
        self.0.on_end(span);
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown()
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown_with_timeout(timeout)
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.0.set_resource(resource);
    }
}

/// Wrapper around an `IdGenerator` to use in `id_generator`.
#[derive(Debug)]
pub(crate) struct BoxedIdGenerator(Box<dyn IdGenerator>);

impl BoxedIdGenerator {
    pub fn new(generator: Box<dyn IdGenerator>) -> Self {
        BoxedIdGenerator(generator)
    }
}

impl IdGenerator for BoxedIdGenerator {
    fn new_trace_id(&self) -> opentelemetry::trace::TraceId {
        self.0.new_trace_id()
    }

    fn new_span_id(&self) -> opentelemetry::trace::SpanId {
        self.0.new_span_id()
    }
}

/// Wrapper around a `MetricReader` to use in `additional_readers`.
#[derive(Debug)]
pub(crate) struct BoxedMetricReader(Box<dyn MetricReader>);

impl BoxedMetricReader {
    pub fn new(reader: Box<dyn MetricReader>) -> Self {
        BoxedMetricReader(reader)
    }
}

impl MetricReader for BoxedMetricReader {
    fn register_pipeline(&self, pipeline: std::sync::Weak<opentelemetry_sdk::metrics::Pipeline>) {
        self.0.register_pipeline(pipeline);
    }

    fn collect(
        &self,
        rm: &mut opentelemetry_sdk::metrics::data::ResourceMetrics,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.collect(rm)
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown()
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown_with_timeout(timeout)
    }

    fn temporality(
        &self,
        kind: opentelemetry_sdk::metrics::InstrumentKind,
    ) -> opentelemetry_sdk::metrics::Temporality {
        self.0.temporality(kind)
    }
}

/// Boxed log processor for dynamic dispatch
#[derive(Debug)]
pub(crate) struct BoxedLogProcessor(Box<dyn LogProcessor + Send + Sync>);

impl BoxedLogProcessor {
    /// Create a new boxed log processor.
    pub fn new(processor: Box<dyn LogProcessor + Send + Sync>) -> Self {
        Self(processor)
    }
}

impl LogProcessor for BoxedLogProcessor {
    fn emit(
        &self,
        log_record: &mut opentelemetry_sdk::logs::SdkLogRecord,
        instrumentation_scope: &opentelemetry::InstrumentationScope,
    ) {
        self.0.emit(log_record, instrumentation_scope);
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown_with_timeout(timeout)
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown()
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.0.set_resource(resource);
    }
}

pub(crate) trait ParseConfigValue: Sized {
    fn parse_config_value(s: &str) -> Result<Self, ConfigureError>;
}

impl<T> ParseConfigValue for T
where
    T: FromStr,
    ConfigureError: From<T::Err>,
{
    fn parse_config_value(s: &str) -> Result<Self, ConfigureError> {
        Ok(s.parse()?)
    }
}

pub(crate) struct ConfigValue<T> {
    env_vars: &'static [&'static str],
    default_value: fn() -> T,
}

impl<T> ConfigValue<T> {
    const fn new(env_vars: &'static [&'static str], default_value: fn() -> T) -> Self {
        Self {
            env_vars,
            default_value,
        }
    }
}
impl<T: ParseConfigValue> ConfigValue<T> {
    /// Resolves a config value, using the provided value if present, otherwise falling back to the environment variable or the default.
    pub(crate) fn resolve(
        &self,
        value: Option<T>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<T, ConfigureError> {
        if let Some(v) = try_resolve_from_env(value, self.env_vars, env)? {
            return Ok(v);
        }

        Ok((self.default_value)())
    }
}

pub(crate) struct OptionalConfigValue<T> {
    env_vars: &'static [&'static str],
    default_value: PhantomData<Option<T>>,
}

impl<T> OptionalConfigValue<T> {
    const fn new(env_vars: &'static [&'static str]) -> Self {
        Self {
            env_vars,
            default_value: PhantomData,
        }
    }
}

impl<T: ParseConfigValue> OptionalConfigValue<T> {
    /// Resolves an optional config value, using the provided value if present, otherwise falling back to the environment variable or `None`.
    pub(crate) fn resolve(
        &self,
        value: Option<T>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<Option<T>, ConfigureError> {
        try_resolve_from_env(value, self.env_vars, env)
    }
}

fn try_resolve_from_env<T>(
    value: Option<T>,
    env_vars: &[&str],
    env: Option<&HashMap<String, String>>,
) -> Result<Option<T>, ConfigureError>
where
    T: ParseConfigValue,
{
    if let Some(v) = value {
        return Ok(Some(v));
    }

    for var in env_vars {
        if let Some(s) = get_optional_env(var, env)? {
            return T::parse_config_value(&s).map(Some);
        }
    }

    Ok(None)
}

impl From<Infallible> for ConfigureError {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible cannot be constructed")
    }
}

pub(crate) static LOGFIRE_SEND_TO_LOGFIRE: ConfigValue<SendToLogfire> =
    ConfigValue::new(&["LOGFIRE_SEND_TO_LOGFIRE"], || SendToLogfire::Yes);

pub(crate) static LOGFIRE_SERVICE_NAME: OptionalConfigValue<String> =
    OptionalConfigValue::new(&["LOGFIRE_SERVICE_NAME", "OTEL_SERVICE_NAME"]);

pub(crate) static LOGFIRE_SERVICE_VERSION: OptionalConfigValue<String> =
    OptionalConfigValue::new(&["LOGFIRE_SERVICE_VERSION", "OTEL_SERVICE_VERSION"]);

pub(crate) static LOGFIRE_ENVIRONMENT: OptionalConfigValue<String> =
    OptionalConfigValue::new(&["LOGFIRE_ENVIRONMENT"]);

#[cfg(test)]
mod tests {
    use crate::config::SendToLogfire;

    #[test]
    fn test_send_to_logfire_from_bool() {
        assert_eq!(SendToLogfire::from(true), SendToLogfire::Yes);
        assert_eq!(SendToLogfire::from(false), SendToLogfire::No);
    }

    #[test]
    #[expect(deprecated)]
    fn test_console_options_with_timestamps() {
        let options = super::ConsoleOptions::default().with_include_timestamps(false);
        assert!(!options.include_timestamps);
        let options = super::ConsoleOptions::default().with_include_timestamps(true);
        assert!(options.include_timestamps);
    }

    #[test]
    #[expect(deprecated)]
    fn test_console_option_with_min_log_level() {
        let console_options = super::ConsoleOptions::default();
        assert_eq!(console_options.min_log_level, tracing::Level::INFO);
        let console_options =
            super::ConsoleOptions::default().with_min_log_level(tracing::Level::DEBUG);
        assert_eq!(console_options.min_log_level, tracing::Level::DEBUG);
    }
}
