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
//!     let logfire = logfire::configure()
//!         .install_panic_handler()
//! #        .send_to_logfire(logfire::config::SendToLogfire::IfTokenPresent)
//!         .finish()?;
//!
//!     let _guard = logfire.shutdown_guard();
//!
//!     logfire::info!("Hello world");
//!
//!     // Guard automatically shuts down Logfire when it goes out of scope
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

use thiserror::Error;

use crate::config::LogfireConfigBuilder;

mod macros;

#[cfg(any(docsrs, doctest))]
pub mod usage;

mod bridges;
pub mod config;
pub mod exporters;
mod logfire;
mod metrics;
mod ulid_id_generator;

pub use macros::*;
pub use metrics::*;

pub use crate::bridges::tracing::LogfireTracingLayer;
pub use crate::logfire::Logfire;

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

    /// Error reading credentials file.
    #[error("Error reading credentials file {path}: {error}")]
    CredentialFileError {
        /// The path to the credentials file.
        path: std::path::PathBuf,
        /// The underlying error.
        error: String,
    },

    /// Any other error.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// An error which may arise when shutting down Logfire.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ShutdownError {
    /// The opentelemetry SDK failed to shut down.
    #[error("Failed to shutdown Otel SDK: {0}")]
    OtelError(#[from] opentelemetry_sdk::error::OTelSdkError),
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
///     let logfire = logfire::configure()
///         .install_panic_handler()
/// #        .send_to_logfire(logfire::config::SendToLogfire::IfTokenPresent)
///         .finish()?;
///
///     let _guard = logfire.shutdown_guard();
///
///     logfire::info!("Hello world");
///
///     // Guard automatically shuts down Logfire when it goes out of scope
///     Ok(())
/// }
/// ```
pub fn configure() -> LogfireConfigBuilder {
    LogfireConfigBuilder::default()
}

pub use crate::logfire::set_local_logfire;

#[cfg(test)]
mod test_utils;
