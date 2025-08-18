use std::{
    backtrace::Backtrace,
    borrow::Cow,
    collections::HashMap,
    panic::PanicHookInfo,
    sync::{Arc, Once},
    time::Duration,
};

#[cfg(feature = "data-dir")]
use std::path::{Path, PathBuf};

use opentelemetry::{
    logs::{LoggerProvider as _, Severity},
    trace::TracerProvider,
};
use opentelemetry_sdk::{
    logs::{BatchLogProcessor, SdkLoggerProvider},
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::{BatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider},
};
use tracing::{Subscriber, level_filters::LevelFilter, subscriber::DefaultGuard};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan};

use crate::{
    __macros_impl::LogfireValue,
    ConfigureError, LogfireConfigBuilder, ShutdownError,
    bridges::tracing::LogfireTracingLayer,
    config::{SendToLogfire, get_base_url_from_token},
    internal::{
        env::get_optional_env,
        exporters::console::{ConsoleWriter, create_console_processors},
        logfire_tracer::{GLOBAL_TRACER, LOCAL_TRACER, LogfireTracer},
    },
    ulid_id_generator::UlidIdGenerator,
};

/// A configured Logfire instance which contains configured `opentelemetry`, `tracing` and `log`
/// integrations.
///
/// This instance is created by calling [`logfire::configure()`][crate::configure].
#[derive(Clone)]
pub struct Logfire {
    pub(crate) tracer_provider: SdkTracerProvider,
    pub(crate) tracer: LogfireTracer,
    pub(crate) subscriber: Arc<dyn Subscriber + Send + Sync>,
    pub(crate) meter_provider: SdkMeterProvider,
    pub(crate) logger_provider: SdkLoggerProvider,
    pub(crate) enable_tracing_metrics: bool,
}

impl Logfire {
    /// Create a shutdown guard that will automatically shutdown Logfire when dropped.
    ///
    /// It is recommended to use this guard in the top level of your application to ensure
    /// that Logfire is properly shut down even when exiting due to a panic.
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
    ///     let guard = logfire.shutdown_guard();
    ///
    ///     logfire::info!("Hello world");
    ///
    ///     guard.shutdown()?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn shutdown_guard(self) -> ShutdownGuard {
        ShutdownGuard {
            logfire: Some(self),
        }
    }

    /// Shuts down the Logfire instance.
    ///
    /// This will flush all data to the opentelemetry exporters and then close all
    /// associated resources.
    ///
    /// # Errors
    ///
    /// See [`ConfigureError`] for possible errors.
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.tracer_provider.shutdown()?;
        self.meter_provider.shutdown()?;
        self.logger_provider.shutdown()?;
        Ok(())
    }

    /// Get a tracing layer which can be used to embed this `Logfire` instance into a `tracing_subscriber::Registry`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tracing_subscriber::{Registry, layer::SubscriberExt};
    ///
    /// let logfire = logfire::configure()
    ///    .local()  // use local mode to avoid setting global state
    ///    .finish()
    ///    .expect("Failed to configure logfire");
    ///
    /// let subscriber = tracing_subscriber::registry()
    ///    .with(logfire.tracing_layer());
    ///
    /// tracing::subscriber::set_global_default(subscriber)
    ///    .expect("Failed to set global subscriber");
    ///
    /// logfire::info!("Hello world");
    ///
    /// logfire.shutdown().expect("Failed to shutdown logfire");
    /// ```
    #[must_use]
    pub fn tracing_layer<S>(&self) -> LogfireTracingLayer<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        LogfireTracingLayer::new(self.tracer.clone(), self.enable_tracing_metrics)
    }

    /// Called by `LogfireConfigBuilder::finish()`.
    pub(crate) fn from_config_builder(
        config: LogfireConfigBuilder,
    ) -> Result<Logfire, ConfigureError> {
        let LogfireParts {
            local,
            tracer,
            subscriber,
            tracer_provider,
            meter_provider,
            logger_provider,
            enable_tracing_metrics,
            ..
        } = Self::build_parts(config, None)?;

        if !local {
            tracing::subscriber::set_global_default(subscriber.clone())?;
            let logger = crate::bridges::log::LogfireLogger::init(tracer.clone());
            log::set_logger(logger)?;
            log::set_max_level(logger.max_level());

            GLOBAL_TRACER
                .set(tracer.clone())
                .map_err(|_| ConfigureError::AlreadyConfigured)?;

            let propagator = opentelemetry::propagation::TextMapCompositePropagator::new(vec![
                Box::new(opentelemetry_sdk::propagation::TraceContextPropagator::new()),
                Box::new(opentelemetry_sdk::propagation::BaggagePropagator::new()),
            ]);
            opentelemetry::global::set_text_map_propagator(propagator);

            opentelemetry::global::set_meter_provider(meter_provider.clone());
        }

        Ok(Logfire {
            tracer_provider,
            tracer,
            subscriber,
            meter_provider,
            logger_provider,
            enable_tracing_metrics,
        })
    }

    /// Load token from credentials file if available.
    #[cfg(feature = "data-dir")]
    fn load_token_from_credentials_file(
        data_dir: Option<&Path>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<Option<LogfireCredentials>, ConfigureError> {
        // Determine credentials directory
        let credentials_dir = if let Some(dir) = data_dir {
            Cow::Borrowed(dir)
        } else if let Some(dir) = get_optional_env("LOGFIRE_CREDENTIALS_DIR", env)? {
            Cow::Owned(PathBuf::from(dir))
        } else {
            // Default to .logfire in current directory
            Cow::Borrowed(Path::new(".logfire"))
        };

        let credentials_path = credentials_dir.join("logfire_credentials.json");

        // Return None if file doesn't exist (not an error)
        if !credentials_path.exists() {
            return Ok(None);
        }

        // Read and parse credentials file
        let contents = std::fs::read_to_string(&credentials_path).map_err(|e| {
            ConfigureError::CredentialFileError {
                path: credentials_path.clone(),
                error: e.to_string(),
            }
        })?;

        match serde_json::from_str(&contents) {
            Ok(credentials) => Ok(Some(credentials)),
            Err(e) => Err(ConfigureError::CredentialFileError {
                path: credentials_path.clone(),
                error: format!("JSON parse error: {e}"),
            }),
        }
    }

    #[expect(clippy::too_many_lines)]
    fn build_parts(
        config: LogfireConfigBuilder,
        env: Option<&HashMap<String, String>>,
    ) -> Result<LogfireParts, ConfigureError> {
        let mut token = config.token;
        if token.is_none() {
            token = get_optional_env("LOGFIRE_TOKEN", env)?;
        }

        #[cfg_attr(
            not(feature = "data-dir"),
            expect(unused_mut, reason = "only mutated on data-dir feature")
        )]
        let mut advanced_options = config.advanced.unwrap_or_default();

        // Try loading from credentials file if still no token
        #[cfg(feature = "data-dir")]
        if token.is_none() {
            if let Some(credentials) =
                Self::load_token_from_credentials_file(config.data_dir.as_deref(), env)?
            {
                token = Some(credentials.token);
                advanced_options.base_url = advanced_options
                    .base_url
                    .or(Some(credentials.logfire_api_url));
            }
        }

        let send_to_logfire = match config.send_to_logfire {
            Some(send_to_logfire) => send_to_logfire,
            None => match get_optional_env("LOGFIRE_SEND_TO_LOGFIRE", env)? {
                Some(value) => value.parse()?,
                None => SendToLogfire::Yes,
            },
        };

        let send_to_logfire = match send_to_logfire {
            SendToLogfire::Yes => true,
            SendToLogfire::IfTokenPresent => token.is_some(),
            SendToLogfire::No => false,
        };

        let mut tracer_provider_builder = SdkTracerProvider::builder();
        let mut logger_provider_builder = SdkLoggerProvider::builder();

        if let Some(id_generator) = advanced_options.id_generator {
            tracer_provider_builder = tracer_provider_builder.with_id_generator(id_generator);
        } else {
            tracer_provider_builder =
                tracer_provider_builder.with_id_generator(UlidIdGenerator::new());
        }

        for resource in &advanced_options.resources {
            tracer_provider_builder = tracer_provider_builder.with_resource(resource.clone());
        }

        let mut http_headers: Option<HashMap<String, String>> = None;

        let logfire_base_url = if send_to_logfire {
            let Some(token) = &token else {
                return Err(ConfigureError::TokenRequired);
            };

            http_headers
                .get_or_insert_default()
                .insert("Authorization".to_string(), format!("Bearer {token}"));

            Some(
                advanced_options
                    .base_url
                    .as_deref()
                    .unwrap_or_else(|| get_base_url_from_token(token)),
            )
        } else {
            None
        };

        if let Some(logfire_base_url) = logfire_base_url {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(
                BatchSpanProcessor::builder(crate::exporters::span_exporter(
                    logfire_base_url,
                    http_headers.clone(),
                )?)
                .with_batch_config(
                    BatchConfigBuilder::default()
                        .with_scheduled_delay(Duration::from_millis(500)) // 500 matches Python
                        .build(),
                )
                .build(),
            );
        }

        let console_processors = config
            .console_options
            .map(|o| create_console_processors(Arc::new(ConsoleWriter::new(o))));

        if let Some((span_processor, log_processor)) = console_processors {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(span_processor);
            logger_provider_builder = logger_provider_builder.with_log_processor(log_processor);
        }

        for span_processor in config.additional_span_processors {
            tracer_provider_builder = tracer_provider_builder.with_span_processor(span_processor);
        }

        let tracer_provider = tracer_provider_builder.build();

        let tracer = tracer_provider.tracer("logfire");
        let default_level_filter = config.default_level_filter.unwrap_or(if send_to_logfire {
            // by default, send everything to the logfire platform, for best UX
            LevelFilter::TRACE
        } else {
            // but if printing locally, just set INFO
            LevelFilter::INFO
        });

        let filter = tracing_subscriber::EnvFilter::builder()
            .with_default_directive(default_level_filter.into())
            .from_env()?; // but allow the user to override this with `RUST_LOG`

        let subscriber = tracing_subscriber::registry().with(filter);

        let mut meter_provider_builder = SdkMeterProvider::builder();

        if let Some(logfire_base_url) = logfire_base_url {
            if config.metrics.is_some() {
                let metric_reader = PeriodicReader::builder(crate::exporters::metric_exporter(
                    logfire_base_url,
                    http_headers.clone(),
                )?)
                .build();

                meter_provider_builder = meter_provider_builder.with_reader(metric_reader);
            }
        }

        if let Some(metrics) = config.metrics {
            for reader in metrics.additional_readers {
                meter_provider_builder = meter_provider_builder.with_reader(reader);
            }
        }

        for resource in &advanced_options.resources {
            meter_provider_builder = meter_provider_builder.with_resource(resource.clone());
        }

        let meter_provider = meter_provider_builder.build();

        if let Some(logfire_base_url) = logfire_base_url {
            logger_provider_builder = logger_provider_builder.with_log_processor(
                BatchLogProcessor::builder(crate::exporters::log_exporter(
                    logfire_base_url,
                    http_headers.clone(),
                )?)
                .build(),
            );
        }

        for log_processor in advanced_options.log_record_processors {
            logger_provider_builder = logger_provider_builder.with_log_processor(log_processor);
        }

        for resource in advanced_options.resources {
            logger_provider_builder = logger_provider_builder.with_resource(resource);
        }

        let logger_provider = logger_provider_builder.build();

        let logger = Arc::new(logger_provider.logger("logfire"));

        let mut filter_builder = env_filter::Builder::new();
        if let Ok(filter) = std::env::var("RUST_LOG") {
            filter_builder.parse(&filter);
        } else {
            filter_builder.parse(&default_level_filter.to_string());
        }

        let tracer = LogfireTracer {
            inner: tracer,
            meter_provider: meter_provider.clone(),
            logger,
            handle_panics: config.install_panic_handler,
            filter: Arc::new(filter_builder.build()),
        };

        let subscriber = subscriber.with(LogfireTracingLayer::new(
            tracer.clone(),
            advanced_options.enable_tracing_metrics,
        ));

        if config.install_panic_handler {
            install_panic_handler();
        }

        Ok(LogfireParts {
            local: config.local,
            tracer,
            subscriber: Arc::new(subscriber),
            tracer_provider,
            meter_provider,
            logger_provider,
            enable_tracing_metrics: advanced_options.enable_tracing_metrics,
            #[cfg(test)]
            metadata: TestMetadata {
                send_to_logfire,
                logfire_token: token,
                logfire_base_url: logfire_base_url.map(str::to_string),
            },
        })
    }
}

/// A guard that automatically shuts down Logfire when dropped.
///
/// Create this guard by calling [`Logfire::shutdown_guard()`] to ensure clean shutdown
/// when the guard goes out of scope.
#[must_use = "this should be kept alive until logging should be stopped"]
pub struct ShutdownGuard {
    logfire: Option<Logfire>,
}

impl ShutdownGuard {
    /// Shutdown the Logfire instance.
    ///
    /// This will flush all spans and metrics to the exporter.
    ///
    /// # Errors
    ///
    /// See [`ShutdownError`] for possible errors.
    pub fn shutdown(mut self) -> Result<(), ShutdownError> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<(), ShutdownError> {
        if let Some(logfire) = self.logfire.take() {
            logfire.shutdown()?;
        }
        Ok(())
    }
}

#[allow(clippy::print_stderr)]
impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_inner() {
            eprintln!("failed to shutdown logfire cleanly: {error:#?}");
        }
    }
}

struct LogfireParts {
    local: bool,
    tracer: LogfireTracer,
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
    enable_tracing_metrics: bool,
    #[cfg(test)]
    metadata: TestMetadata,
}

#[cfg(test)]
struct TestMetadata {
    send_to_logfire: bool,
    logfire_token: Option<String>,
    logfire_base_url: Option<String>,
}

/// Install `handler` as part of a chain of panic handlers.
fn install_panic_handler() {
    fn panic_hook(info: &PanicHookInfo) {
        LogfireTracer::try_with(|tracer| {
            if !tracer.handle_panics {
                // this tracer is not handling panics
                return;
            }

            let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
                s
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s
            } else {
                ""
            };

            let location = info.location();
            tracer.export_log(
                None,
                &tracing::Span::current().context(),
                format!("panic: {message}"),
                Severity::Error,
                crate::__json_schema!(backtrace),
                location.map(|l| Cow::Owned(l.file().to_string())),
                location.map(std::panic::Location::line),
                None,
                [LogfireValue::new(
                    "backtrace",
                    Some(Backtrace::capture().to_string().into()),
                )],
            );
        });
    }

    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            panic_hook(info);
            prev(info);
        }));
    });
}

/// Helper for installing a logfire guard locally to a thread.
///
/// FIXME: This is still a bit of a mess, it's only implemented far enough to make tests pass...
#[doc(hidden)]
pub struct LocalLogfireGuard {
    prior: Option<LogfireTracer>,
    #[expect(dead_code, reason = "tracing RAII guard")]
    tracing_guard: DefaultGuard,
    logfire: Logfire,
}

impl LocalLogfireGuard {
    /// Get the current tracer.
    #[must_use]
    pub fn subscriber(&self) -> Arc<dyn Subscriber + Send + Sync> {
        self.logfire.subscriber.clone()
    }

    /// Get the current meter provider
    #[must_use]
    pub fn meter_provider(&self) -> &SdkMeterProvider {
        &self.logfire.meter_provider
    }

    /// Convenience function to release this guard and shutdown Logfire.
    pub fn shutdown(self) -> Result<(), ShutdownError> {
        let logfire = self.logfire.clone();
        drop(self); // ensure the guard is dropped before shutdown
        logfire.shutdown()
    }
}

impl Drop for LocalLogfireGuard {
    fn drop(&mut self) {
        // FIXME: if drop order is not consistent with creation order, does this create strange
        // state?
        LOCAL_TRACER.with_borrow_mut(|local_logfire| {
            *local_logfire = self.prior.take();
        });
    }
}

#[doc(hidden)] // used in tests
#[must_use]
pub fn set_local_logfire(logfire: Logfire) -> LocalLogfireGuard {
    let prior =
        LOCAL_TRACER.with_borrow_mut(|local_logfire| local_logfire.replace(logfire.tracer.clone()));

    let tracing_guard = tracing::subscriber::set_default(logfire.subscriber.clone());

    // TODO: metrics??

    LocalLogfireGuard {
        prior,
        tracing_guard,
        logfire,
    }
}

/// Credentials stored in `logfire_credentials.json` files
#[cfg(feature = "data-dir")]
#[derive(serde::Deserialize)]
struct LogfireCredentials {
    token: String,
    #[expect(dead_code, reason = "not used for now")]
    project_name: String,
    #[expect(dead_code, reason = "not used for now")]
    project_url: String,
    logfire_api_url: String,
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use opentelemetry_sdk::trace::{SpanData, SpanProcessor};

    use crate::{ConfigureError, Logfire, config::SendToLogfire, configure};

    #[test]
    fn test_send_to_logfire() {
        for (env, setting, expected) in [
            (vec![], None, Err(ConfigureError::TokenRequired)),
            (vec![("LOGFIRE_TOKEN", "a")], None, Ok(true)),
            (vec![("LOGFIRE_SEND_TO_LOGFIRE", "no")], None, Ok(false)),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "yes")],
                None,
                Err(ConfigureError::TokenRequired),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "yes"), ("LOGFIRE_TOKEN", "a")],
                None,
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "if-token-present")],
                None,
                Ok(false),
            ),
            (
                vec![
                    ("LOGFIRE_SEND_TO_LOGFIRE", "if-token-present"),
                    ("LOGFIRE_TOKEN", "a"),
                ],
                None,
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no"), ("LOGFIRE_TOKEN", "a")],
                Some(SendToLogfire::Yes),
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no"), ("LOGFIRE_TOKEN", "a")],
                Some(SendToLogfire::IfTokenPresent),
                Ok(true),
            ),
            (
                vec![("LOGFIRE_SEND_TO_LOGFIRE", "no")],
                Some(SendToLogfire::IfTokenPresent),
                Ok(false),
            ),
        ] {
            let env: std::collections::HashMap<String, String> =
                env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();

            let mut config = crate::configure();
            if let Some(value) = setting {
                config = config.send_to_logfire(value);
            }

            let md = Logfire::build_parts(config, Some(&env)).map(|parts| parts.metadata);

            if let Ok(md) = &md {
                assert!(!md.send_to_logfire || md.logfire_token.is_some());
                assert!(
                    !md.send_to_logfire
                        || md.logfire_base_url.as_deref()
                            == Some("https://logfire-us.pydantic.dev")
                );
            }

            let result = md.as_ref().map(|md| md.send_to_logfire);

            match (expected, result) {
                (Ok(exp), Ok(actual)) => assert_eq!(exp, actual),
                // compare strings because ConfigureError doesn't implement PartialEq
                (Err(exp), Err(actual)) => assert_eq!(exp.to_string(), actual.to_string()),
                (expected, result) => panic!("expected {expected:?}, got {result:?}"),
            }
        }
    }

    #[derive(Debug)]
    struct TestShutdownProcessor {
        shutdown_called: Arc<AtomicBool>,
    }

    impl SpanProcessor for TestShutdownProcessor {
        fn on_start(&self, _: &mut opentelemetry_sdk::trace::Span, _: &opentelemetry::Context) {}

        fn on_end(&self, _: SpanData) {}

        fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
            Ok(())
        }

        fn shutdown_with_timeout(&self, _: Duration) -> opentelemetry_sdk::error::OTelSdkResult {
            self.shutdown_called.store(true, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn test_shutdown_guard_drop() {
        let shutdown_called = Arc::new(AtomicBool::new(false));

        {
            let logfire = configure()
                .local()
                .send_to_logfire(false)
                .with_additional_span_processor(TestShutdownProcessor {
                    shutdown_called: shutdown_called.clone(),
                })
                .finish()
                .expect("failed to configure logfire");

            let _guard = logfire.shutdown_guard();

            // Guard is alive here, shutdown should not have been called
            assert!(!shutdown_called.load(Ordering::Relaxed));
        }

        // Guard is dropped here, shutdown should be called
        assert!(shutdown_called.load(Ordering::Relaxed));
    }

    #[test]
    fn test_drop_multiple_shutdown_guard() {
        let shutdown_called = Arc::new(AtomicBool::new(false));

        let logfire = configure()
            .local()
            .send_to_logfire(false)
            .with_additional_span_processor(TestShutdownProcessor {
                shutdown_called: shutdown_called.clone(),
            })
            .finish()
            .expect("failed to configure logfire");

        // Having multiple shutdown guards should not cause issues when they are dropped
        {
            let _guard1 = logfire.clone().shutdown_guard();
            let _guard2 = logfire.shutdown_guard();

            // Guards are alive here, shutdown should not have been called
            assert!(!shutdown_called.load(Ordering::Relaxed));
        }

        // Guards have been dropped, shutdown should be called
        assert!(shutdown_called.load(Ordering::Relaxed));
    }

    #[test]
    fn test_manual_shutdown() {
        let shutdown_called = Arc::new(AtomicBool::new(false));

        let logfire = configure()
            .local()
            .send_to_logfire(false)
            .with_additional_span_processor(TestShutdownProcessor {
                shutdown_called: shutdown_called.clone(),
            })
            .finish()
            .expect("failed to configure logfire");

        // Not shutdown yet
        assert!(!shutdown_called.load(Ordering::Relaxed));

        // Manual shutdown should work
        logfire.shutdown().expect("shutdown should succeed");

        // Shutdown should have been called
        assert!(shutdown_called.load(Ordering::Relaxed));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    #[should_panic(expected = "OtelError(AlreadyShutdown)")]
    fn test_multiple_shutdown_calls() {
        let logfire = configure()
            .local()
            .send_to_logfire(false)
            .finish()
            .expect("failed to configure logfire");

        // First `shutdown` call should succeed
        logfire.shutdown().expect("first shutdown should succeed");

        // Second `shutdown` call should fail
        logfire.shutdown().unwrap();
    }

    #[test]
    #[cfg(feature = "data-dir")]
    fn test_credentials_file_loading() {
        const CREDENTIALS_JSON: &str = r#"{
    "token": "test_token_123",
    "project_name": "test-project",
    "project_url": "https://logfire-eu.pydantic.dev/test-org/test-project",
    "logfire_api_url": "https://test-api-url.com"
}"#;

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        // Write credentials file
        let credentials_path = temp_dir.path().join("logfire_credentials.json");
        std::fs::write(&credentials_path, CREDENTIALS_JSON).unwrap();

        // Test that credentials are loaded when using with_data_dir
        let config = crate::configure().local().with_data_dir(temp_dir.path());

        let md = Logfire::build_parts(config, None).unwrap().metadata;

        assert_eq!(md.send_to_logfire, true);
        assert_eq!(md.logfire_token.as_deref(), Some("test_token_123"));
        assert_eq!(
            md.logfire_base_url.as_deref(),
            Some("https://test-api-url.com")
        );
    }

    #[test]
    #[cfg(feature = "data-dir")]
    fn test_credentials_file_loading_env_var() {
        const CREDENTIALS_JSON: &str = r#"{
    "token": "test_token_123",
    "project_name": "test-project",
    "project_url": "https://logfire-eu.pydantic.dev/test-org/test-project",
    "logfire_api_url": "https://test-api-url.com"
}"#;

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        // Write credentials file
        let credentials_path = temp_dir.path().join("logfire_credentials.json");
        std::fs::write(&credentials_path, CREDENTIALS_JSON).unwrap();

        // Test that credentials are loaded when using with_data_dir
        let config = crate::configure().local();

        let env: std::collections::HashMap<String, String> = [(
            "LOGFIRE_CREDENTIALS_DIR".to_string(),
            temp_dir.path().display().to_string(),
        )]
        .into_iter()
        .collect();

        let md = Logfire::build_parts(config, Some(&env)).unwrap().metadata;

        assert_eq!(md.send_to_logfire, true);
        assert_eq!(md.logfire_token.as_deref(), Some("test_token_123"));
        assert_eq!(
            md.logfire_base_url.as_deref(),
            Some("https://test-api-url.com")
        );
    }

    #[test]
    #[cfg(feature = "data-dir")]
    fn test_credentials_file_error_handling() {
        // Create a temporary directory using tempfile
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let credentials_path = temp_dir.path().join("logfire_credentials.json");

        // Test with invalid JSON
        std::fs::write(&credentials_path, "invalid json").unwrap();

        let result = crate::configure()
            .local()
            .send_to_logfire(true) // This should require a token
            .with_data_dir(temp_dir.path())
            .finish();

        // Should get a credential file error due to invalid JSON
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(
                e,
                crate::ConfigureError::CredentialFileError { .. }
            ));
        }
        // temp_dir is cleaned up automatically
    }

    #[test]
    #[cfg(feature = "data-dir")]
    fn test_no_credentials_file_fallback() {
        // Create a temporary directory without any credentials file using tempfile
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = crate::configure()
            .local()
            .send_to_logfire(true) // This should require a token
            .with_data_dir(temp_dir.path())
            .finish();

        // Should get TokenRequired error since no credentials file exists
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e, crate::ConfigureError::TokenRequired));
        }
        // temp_dir is cleaned up automatically
    }
}
