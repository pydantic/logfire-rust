//! Configuration options for the Logfire SDK.
//!
//! See [`LogfireConfigBuilder`][crate::LogfireConfigBuilder] for documentation of all these options.

use std::{
    fmt::Display,
    str::FromStr,
    sync::{Arc, Mutex},
};

use opentelemetry_sdk::{
    metrics::reader::MetricReader,
    trace::{IdGenerator, SpanProcessor},
};
use regex::Regex;

use crate::ConfigureError;
use tracing::Level;

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
    /// Where to send output
    pub(crate) target: Target,
    /// Whether to include timestamps in the console output.
    pub include_timestamps: bool,
    /// The minimum log level to show in the console.
    pub min_log_level: Level,
    // TODO: support the below configuration options (inherited from Python SDK)

    // /// Whether to show colors in the console.
    // colors: ConsoleColors,
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
            // colors: ConsoleColors::default(),
            // span_style: SpanStyle::default(),
            // include_tags: true,
            // verbose: false,
            min_log_level: Level::INFO,
            // show_project_link: true,
            target: Target::default(),
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
    /// Set whether to include timestamps in the console output.
    #[must_use]
    pub fn with_include_timestamps(mut self, include: bool) -> Self {
        self.include_timestamps = include;
        self
    }

    /// Set the minimum log level to show in the console.
    #[must_use]
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
    /// locking, however performance sensitive use cases might want to just use
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

/// Options primarily used for testing by Logfire developers.
#[derive(Default)]
pub struct AdvancedOptions {
    /// Root URL for the Logfire API.
    pub(crate) base_url: Option<String>,
    /// Generator for trace and span IDs.
    pub(crate) id_generator: Option<BoxedIdGenerator>,
    /// Resource to override default resource detection.
    pub(crate) resource: Option<opentelemetry_sdk::Resource>,
    //
    //
    // TODO: arguments below supported by Python

    // /// Generator for nanosecond start and end timestamps of spans.
    // pub ns_timestamp_generator: Option,

    // /// Configuration for OpenTelemetry logging. This is experimental and may be removed.
    // pub log_record_processors: Vec<Box<dyn LogRecordProcessor>>,
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

    /// Set the resource; overrides default resource detection.
    #[must_use]
    pub fn with_resource(mut self, resource: opentelemetry_sdk::Resource) -> Self {
        self.resource = Some(resource);
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
    ) -> opentelemetry_sdk::metrics::MetricResult<()> {
        self.0.collect(rm)
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown()
    }

    fn temporality(
        &self,
        kind: opentelemetry_sdk::metrics::InstrumentKind,
    ) -> opentelemetry_sdk::metrics::Temporality {
        self.0.temporality(kind)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SendToLogfire;

    #[test]
    fn test_send_to_logfire_from_bool() {
        assert_eq!(SendToLogfire::from(true), SendToLogfire::Yes);
        assert_eq!(SendToLogfire::from(false), SendToLogfire::No);
    }

    #[test]
    fn test_console_options_with_timestamps() {
        let options = super::ConsoleOptions::default().with_include_timestamps(false);
        assert!(!options.include_timestamps);
        let options = super::ConsoleOptions::default().with_include_timestamps(true);
        assert!(options.include_timestamps);
    }

    #[test]
    fn test_console_option_with_min_log_level() {
        let console_options = super::ConsoleOptions::default();
        assert_eq!(console_options.min_log_level, tracing::Level::INFO);
        let console_options =
            super::ConsoleOptions::default().with_min_log_level(tracing::Level::DEBUG);
        assert_eq!(console_options.min_log_level, tracing::Level::DEBUG);
    }

    #[tokio::test]
    async fn test_console_with_tokio_sleep() {
        // token is invalid, so exports will fail, but it was a necessary part of reproducing the issue
        let shutdown_handler = crate::configure()
            .local()
            .with_token("abc123")
            .install_panic_handler()
            .finish()
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        shutdown_handler.shutdown().ok();
    }
}
