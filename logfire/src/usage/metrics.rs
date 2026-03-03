//! # Exporting OpenTelemetry Metrics from Rust to Pydantic Logfire
//!
//! This guide shows how to export OpenTelemetry metrics from Rust applications to Pydantic Logfire and use them for creating dashboards and monitoring.
//!
//! ## Overview
//!
//! The Logfire Rust SDK provides built-in support for OpenTelemetry metrics through its metrics API. Metrics are automatically exported to Logfire alongside traces and logs, giving you comprehensive observability for your Rust applications.
//!
//! ## Prerequisites
//!
//! 1. **Set up a Logfire project** at [logfire.pydantic.dev](https://logfire.pydantic.dev/docs/#logfire)
//! 2. **Create a write token** following the [token creation guide](https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens/)
//! 3. **Set the environment variable**: `export LOGFIRE_TOKEN=your_token_here`
//!
//! ## Available Metric Types
//!
//! The Logfire SDK supports all OpenTelemetry metric types:
//!
//! ### Counters
//!
//! Monotonically increasing values (e.g., request counts, error counts):
//!
//! ```rust
//! use std::sync::LazyLock;
//! use opentelemetry::metrics::Counter;
//!
//! // For counting discrete events
//! static HTTP_REQUESTS: LazyLock<Counter<u64>> = LazyLock::new(|| {
//!     logfire::u64_counter("http_requests_total")
//!         .with_description("Total HTTP requests")
//!         .with_unit("{request}")
//!         .build()
//! });
//!
//! // For floating-point measurements
//! static DATA_PROCESSED: LazyLock<Counter<f64>> = LazyLock::new(|| {
//!     logfire::f64_counter("data_processed_bytes")
//!         .with_description("Total bytes processed")
//!         .with_unit("By")
//!         .build()
//! });
//! ```
//!
//! ### Gauges
//!
//! Current state values that can go up or down:
//!
//! ```rust
//! use std::sync::LazyLock;
//! use opentelemetry::metrics::Gauge;
//!
//! static ACTIVE_CONNECTIONS: LazyLock<Gauge<u64>> = LazyLock::new(|| {
//!     logfire::u64_gauge("active_connections")
//!         .with_description("Number of active connections")
//!         .with_unit("{connection}")
//!         .build()
//! });
//!
//! static CPU_USAGE: LazyLock<Gauge<f64>> = LazyLock::new(|| {
//!     logfire::f64_gauge("cpu_usage_percent")
//!         .with_description("CPU usage percentage")
//!         .with_unit("%")
//!         .build()
//! });
//! ```
//!
//! ### Histograms
//!
//! Distribution of values (e.g., request durations, response sizes):
//!
//! ```rust
//! use std::sync::LazyLock;
//! use opentelemetry::metrics::Histogram;
//! use logfire::ExponentialHistogram;
//!
//! static REQUEST_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
//!     logfire::f64_histogram("http_request_duration")
//!         .with_description("HTTP request duration")
//!         .with_unit("s")
//!         .build()
//! });
//!
//! static RESPONSE_SIZE: LazyLock<Histogram<u64>> = LazyLock::new(|| {
//!     logfire::u64_histogram("http_response_size")
//!         .with_description("HTTP response size")
//!         .with_unit("By")
//!         .build()
//! });
//!
//! static QUEUE_LATENCY: LazyLock<ExponentialHistogram<f64>> = LazyLock::new(|| {
//!     logfire::f64_exponential_histogram("queue_latency", 20)
//!         .with_description("Latency of items in a queue")
//!         .with_unit("ms")
//!         .build()
//! });
//!
//! static MEMORY_USAGE_BYTES: LazyLock<ExponentialHistogram<u64>> = LazyLock::new(|| {
//!     logfire::u64_exponential_histogram("memory_usage_bytes", 20)
//!         .with_description("Memory usage in bytes")
//!         .with_unit("By")
//!         .build()
//! });
//! ```
//!
//! ### Up/Down Counters
//!
//! Values that can increase or decrease:
//!
//! ```rust
//! use std::sync::LazyLock;
//! use opentelemetry::metrics::UpDownCounter;
//!
//! static QUEUE_SIZE: LazyLock<UpDownCounter<i64>> = LazyLock::new(|| {
//!     logfire::i64_up_down_counter("queue_size")
//!         .with_description("Current queue size")
//!         .with_unit("{item}")
//!         .build()
//! });
//! ```
//!
//! ## Observable Metrics
//!
//! For metrics that need to be sampled periodically rather than recorded on-demand:
//!
//! ```rust
//! use std::sync::LazyLock;
//! use opentelemetry::metrics::ObservableGauge;
//!
//! static MEMORY_USAGE: LazyLock<ObservableGauge<u64>> = LazyLock::new(|| {
//!     logfire::u64_observable_gauge("memory_usage_bytes")
//!         .with_description("Current memory usage")
//!         .with_unit("By")
//!         .with_callback(|observer| {
//!             // Get current memory usage (example)
//!             let memory_usage = get_memory_usage();
//!             observer.observe(memory_usage, &[]);
//!         })
//!         .build()
//! });
//!
//! fn get_memory_usage() -> u64 {
//!     // Implementation to get actual memory usage
//!     1024 * 1024 * 100 // Example: 100MB
//! }
//! ```
//!
//! ## Examples
//!
//! For examples using these metrics, see the [examples directory][crate::usage::examples].
//!
//! ## Using Metrics in Logfire Dashboards
//!
//! Once your metrics are being exported to Logfire, you can create dashboards and alerts. To check metrics
//! arriving in Logfire, you can use the following steps:
//!
//! 1. Navigate to your Logfire project dashboard
//! 2. Go to the ["Explore"](https://logfire.pydantic.dev/docs/guides/web-ui/explore/) view
//! 3. Run `select * from metrics` to view all incoming metrics from your application
//!
//! ## Best Practices
//!
//! 1. **Use Static Variables**: Define metrics as static variables with `LazyLock` for efficiency
//! 2. **Meaningful Names**: Use descriptive metric names following OpenTelemetry conventions
//! 3. **Consistent Labels**: Use consistent label names across related metrics
//! 4. **Appropriate Units**: Specify units (seconds, bytes, requests) for better dashboard formatting
//! 5. **Documentation**: Add descriptions to help with dashboard creation
//! 6. **Label Cardinality**: Be careful with high-cardinality labels (e.g., user IDs) - prefer aggregated metrics
//!
//! ## `tracing-opentelemetry` metrics
//!
//! `tracing-opentelemetry` has its own
//! [`MetricsLayer`](https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/struct.MetricsLayer.html) which
//! can be used to automatically produce metrics from `tracing` events.
//!
//! Reporting metrics through this pattern adds a level of indirection compared to the direct use of the logfire metrics API described
//! above. In some cases, however, this may be useful, or necessary if dependencies are emitting
//! metrics in this form.
//!
//! This SDK provides a built-in way to handle these metrics via [`AdvancedOptions::with_tracing_metrics`][crate::config::AdvancedOptions::with_tracing_metrics].
//!
//! ## Troubleshooting
//!
//! ### Metrics Not Appearing
//!
//! - Verify `LOGFIRE_TOKEN` is set correctly
//! - Check that metrics are being recorded (add debug logging)
//! - Ensure the application runs long enough for metrics to be exported
//!
//! ### Performance Issues
//!
//! - Monitor label cardinality - too many unique label combinations can impact performance
//! - Use histograms instead of gauges for distributions
//! - Consider sampling for high-frequency metrics
//!
//! ### Dashboard Issues
//!
//! - Check metric names match exactly (case-sensitive)
//! - Verify time ranges in queries
//! - Use Logfire's query builder to test metric availability
//!
//! This guide provides a complete foundation for implementing metrics in your Rust applications and visualizing them in Logfire dashboards.
