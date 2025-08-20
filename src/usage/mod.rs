//! # Usage Guide
//!
//! This section of the document is dedicated to guide material which shows how to
//! use the Pydantic Logfire Rust SDK to instrument applications.
//!
//! # Architecture
//!
//! This section briefly documents how this SDK is built upon other libraries.
//!
//! ## Integrations
//!
//! This crate has integrations with the following libraries:
//!
//! ### With `tracing`
//!
//! This SDK is built upon `tracing` (and `tracing-opentelemetry`) for the [`logfire::span!`][crate::span] macro. This means
//! that any code instrumented with `tracing` will automatically be captured by Logfire, and also
//! that [`logfire::span!`][crate::span] produces a `tracing::Span` which is fully compatible with the `tracing` ecosystem.
//!
//! If you are an existing `tracing` user, it is fine to continue to use the `tracing` APIs directly
//! and ignore [`logfire::span!`][crate::span]. The upside of [`logfire::span!`][crate::span] is that it will show the fields
//! directly in the logfire UI.
//!
//! There are many great APIs in `tracing` which we do not yet provide equivalents for, such as the
//! [`#[tracing::instrument]`][`macro@tracing::instrument`] proc macro, so even if using [`logfire::span!`][crate::span]
//! you will likely use `tracing` APIs directly too.
//!
//! ### With `opentelemetry`
//!
//! This SDK is built upon the `opentelemetry` Rust SDK and will configure the global `opentelemetry`
//! state as part of a call to [`logfire::configure()`][crate::configure].
//!
//! All calls to [`logfire::info!`][crate::info] and similar macros are directly forwarded to `opentelemetry`
//! machinery without going through `tracing`, for performance.
//!
//! The metrics helpers exported by this SDK, such as [`logfire::u64_counter()`][crate::u64_counter], are
//! very thin wrappers around the `opentelemetry` SDK.
//!
//! ### With `log`
//!
//! This SDK configures the global `log` state to use an exporter which forwards logs to opentelemetry.
//!
//! All code instrumented with `log` will therefore automatically be captured by Logfire.
//!
//! # Dynamically adding data to spans
//!
//! Occasionally you may want to add data to a span after it has been created.
//!
//! This works the same way as with `tracing`, using the `record` method on the span. When creating
//! the span, you need to initialize the attribute with [`tracing::field::Empty`].
//!
//! ```rust
//! // 1. create span with placeholder attribute
//! let span = logfire::span!("My span", my_attr = tracing::field::Empty);
//!
//! // 2. record data later
//! span.record("my_attr", "some value");
//! ```
//!
//! # Handling panics
//!
//! By default, the Logfire SDK will install a [panic hook][std::panic::set_hook] to record panics as error logs.
//!
//! In the case that the panic is causing the application to fully unwind and exit, you should
//! ensure that there is a [`ShutdownGuard`][crate::ShutdownGuard] installed on your application's main stack frame.
//!
//! All [examples] follow this best-practice.
//!
//! # Using `logfire` as a layer in an existing `tracing` application
//!
//! If you have an existing application which already has a [`tracing_subscriber`] and you
//! want to use `logfire` to quickly configure the OpenTelemetry SDK and send traces and logs to Logfire, the
//! [`LogfireTracingLayer`][crate::LogfireTracingLayer] can be used to achieve this.
//!
//! First, configure `logfire` as usual, setting it as a `.local()` instance to avoid configuring
//! global state:
//!
//! ```rust
//! use tracing_subscriber::prelude::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. configure logfire as usual, setting it as a `.local()` instance
//! let logfire = logfire::configure()
//!     .local()
//!     .finish()?;
//!
//! // 2. add a logfire shutdown guard for panics
//! let _guard = logfire.shutdown_guard();
//!
//! // 3. create a tracing subscriber
//! let subscriber = tracing_subscriber::registry()
//!     .with(logfire.tracing_layer());
//!
//! // 4. set the subscriber as the default (or otherwise set it up for your application)
//! tracing::subscriber::set_global_default(subscriber)?;
//!
//! // 5. now tracing's spans and logs will be sent to Logfire
//! tracing::info!("This will be sent to Logfire");
//!
//! // 6. when finished, call logfire.shutdown() to flush and clean up
//! logfire.shutdown()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Examples
//!
//! See [examples] subchapter of this documentation.

pub mod examples;
pub mod metrics;
