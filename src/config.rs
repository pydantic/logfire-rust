use std::{fmt::Display, str::FromStr};

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
