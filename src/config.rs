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
use tracing::Level;

use crate::ConfigureError;

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
#[expect(clippy::struct_excessive_bools)] // Config options, bools make sense here.
#[derive(Debug, Clone)]
pub struct ConsoleOptions {
    /// Whether to show colors in the console.
    pub colors: ConsoleColors,
    /// How spans are shown in the console.
    pub span_style: SpanStyle,
    /// Whether to include timestamps in the console output.
    pub include_timestamps: bool,
    /// Whether to include tags in the console output.
    pub include_tags: bool,
    /// Whether to show verbose output.
    ///
    /// It includes the filename, log level, and line number.
    pub verbose: bool,
    /// The minimum log level to show in the console.
    pub min_log_level: Level,
    /// Whether to print the URL of the Logfire project after initialization.
    pub show_project_link: bool,
    /// Where to send output
    pub target: Target,
}

impl Default for ConsoleOptions {
    fn default() -> Self {
        ConsoleOptions {
            colors: ConsoleColors::default(),
            span_style: SpanStyle::default(),
            include_timestamps: true,
            include_tags: true,
            verbose: false,
            min_log_level: Level::INFO,
            show_project_link: true,
            target: Target::default(),
        }
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
pub struct AdvancedOptions {
    /// Root URL for the Logfire API.
    pub(crate) base_url: String,
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

impl Default for AdvancedOptions {
    fn default() -> Self {
        AdvancedOptions {
            base_url: "https://logfire-api.pydantic.dev".to_string(),
            id_generator: None,
            resource: None,
        }
    }
}

impl AdvancedOptions {
    /// Set the base URL for the Logfire API.
    #[must_use]
    pub fn with_base_url<T: AsRef<str>>(mut self, base_url: T) -> Self {
        self.base_url = base_url.as_ref().into();
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
}
