//! Configuration options for the Logfire SDK.
//!
//! See [`LogfireConfigBuilder`][crate::LogfireConfigBuilder] for documentation of all these options.

use std::{fmt::Display, str::FromStr};

use opentelemetry_sdk::trace::{IdGenerator, SpanProcessor};
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
        }
    }
}

/// Whether to show colors in the console.
#[derive(Default)]
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
#[derive(Default)]
pub enum SpanStyle {
    /// Show spans in a simple format.
    Simple,
    /// Show spans in a detailed format.
    Indented,
    /// Show parent span ids when printing spans.
    #[default]
    ShowParents,
}

/// Options primarily used for testing by Logfire developers.
pub struct AdvancedOptions {
    /// Root URL for the Logfire API.
    pub(crate) base_url: String,
    /// Generator for trace and span IDs.
    pub(crate) id_generator: Option<BoxedIdGenerator>,
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
