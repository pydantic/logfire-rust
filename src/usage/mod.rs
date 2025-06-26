//! # Usage Guide
//!
//! This section of the document is dedicated to guide material which shows how to
//! use the Logfire Rust SDK to instrument applications.
//!
//! # Architecture
//!
//! This section briefly documents how this SDK is built upon other libraries.
//!
//! ## Integrations
//!
//! The following sections describe briefly the interaction which this SDK has with other libraries.
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
//! # Examples
//!
//! See [examples] subchapter of this documentation.

pub mod examples;
