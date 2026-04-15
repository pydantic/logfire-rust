use opentelemetry_sdk::logs::LogProcessor;

/// Workaround for <https://github.com/open-telemetry/opentelemetry-rust/issues/3132>
///
/// This wraps the inner exporter and redirects calls to `shutdown_with_timeout` to
/// `shutdown`.
#[derive(Debug)]
pub(crate) struct LogProcessorShutdownHack<T>(T);

impl<T> LogProcessorShutdownHack<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T: LogProcessor> LogProcessor for LogProcessorShutdownHack<T> {
    fn emit(
        &self,
        data: &mut opentelemetry_sdk::logs::SdkLogRecord,
        instrumentation: &opentelemetry::InstrumentationScope,
    ) {
        self.0.emit(data, instrumentation);
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown_with_timeout(
        &self,
        _timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        // Calling `.shutdown` here is the deliberate purpose of the abstraction
        self.0.shutdown()
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.0.shutdown()
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.0.set_resource(resource);
    }
}
