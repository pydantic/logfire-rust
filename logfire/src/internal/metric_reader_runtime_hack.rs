use std::time::Duration;

use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::reader::MetricReader;
use opentelemetry_sdk::metrics::{InstrumentKind, Temporality};

/// Pins the background task of an "async runtime" metric reader to a specific tokio runtime.
///
/// The `periodic_reader_with_async_runtime::PeriodicReader` does not spawn its worker task
/// when it is built; it spawns it lazily inside [`MetricReader::register_pipeline`] (called
/// by `SdkMeterProvider::builder().build()`), using whatever tokio runtime is ambient *at
/// that point*.
///
/// Without this wrapper, the worker task would be spawned on the caller's runtime when
/// `logfire::configure()` is called inside a tokio context (causing `shutdown()` to
/// deadlock on single-threaded runtimes, since it blocks the thread while waiting for the
/// worker to respond), or panic outside a tokio context. Entering the handle of logfire's
/// dedicated export runtime here ensures the worker always runs on that runtime, matching
/// the batch span/log processors (which spawn their workers at build time, where
/// `logfire` already enters the export runtime's handle).
#[derive(Debug)]
pub(crate) struct MetricReaderRuntimeHack<T> {
    inner: T,
    handle: tokio::runtime::Handle,
}

impl<T> MetricReaderRuntimeHack<T> {
    pub(crate) fn new(inner: T, handle: tokio::runtime::Handle) -> Self {
        Self { inner, handle }
    }
}

impl<T: MetricReader> MetricReader for MetricReaderRuntimeHack<T> {
    fn register_pipeline(&self, pipeline: std::sync::Weak<opentelemetry_sdk::metrics::Pipeline>) {
        // Entering the runtime handle here is the deliberate purpose of the abstraction;
        // the inner reader spawns its worker task inside `register_pipeline`.
        let _guard = self.handle.enter();
        self.inner.register_pipeline(pipeline);
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        self.inner.collect(rm)
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.inner.shutdown()
    }

    fn temporality(&self, kind: InstrumentKind) -> Temporality {
        self.inner.temporality(kind)
    }
}
