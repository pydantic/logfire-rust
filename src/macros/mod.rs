#[doc(hidden)]
#[path = "impl_.rs"]
pub mod __macros_impl;

/// Create a new [`Span`][tracing::Span]. This macro is a simplified version of `tracing::span!` which accepts
/// a smaller set of possible arguments and syntaxes.
///
/// In exchange, the macro automatically adds metadata which leads the span and its fields
/// to present nicely in the Pydantic Logfire UI.
///
/// # Syntax
///
/// The macro accepts the following syntax:
///
/// ```rust
/// # let value = 42;
/// logfire::span!(
///    parent: tracing::Span::current(), // optional, tracing::Span
///    level: tracing::Level::INFO,      // optional, tracing::Level
///    "format string",                  // required, must be a format string accepted by `format!()`
///    attr = value,                     // optional, attribute key and value pair
///    // ..                             // as many additional attr = value pairs as desired
/// );
/// ```
///
/// ## Attributes
///
/// The `attr = value` pairs are captured as [attributes] on the span.
///
/// `dotted.name = value` is also supported (and encouraged by Opentelemetry) to namespace
/// attributes. However, dotted names are not supported in Rust format strings.
///
/// ## Formatting
///
/// The format string only accepts arguments by name. These can either be explicitly passed
/// to `span!` as an attribute or captured from the surrounding scope by the macro. If captured
/// from the surrounding scope, they will only be used as a format string and not exported
/// as attributes.
///
/// # Examples
///
/// ```rust,no_run
/// // Span without any arguments
/// let root_span = logfire::span!("Root span");
///
/// // Span with attributes x and y
/// let span = logfire::span!(parent: &root_span, "Child span", x = 42, y = "hello");
///
/// // Typically a span will be "entered" to set the parent implicitly
/// root_span.in_scope(|| {
///     // This span will be a child of root_span
///     let child_span = logfire::span!("Nested span", x = 42, y = "hello");
///
///     // Debug-level child span
///     let debug_span = logfire::span!(level: tracing::Level::DEBUG, "Debugging", x = 42, y = "hello");
/// });
///
/// // With x included in the formatted message but not as an attribute
/// let x = 42;
/// let span = logfire::span!("Span with x = {x}, y = {y}", y = "hello");
///
/// // Attributes can either be a single name or a dotted.name
/// // `dotted.name` is not available in the format string.
/// let span = logfire::span!("Span with x = {x}, y = {y}", y = "hello", foo.bar = 42);
///
/// // If just an identifier is used without `= value`, it will be captured as an attribute
/// let foo = "bar";
/// let span = logfire::span!("Span with foo = {foo}", foo);
///
/// // This identifier shorthand also works for struct fields
/// #[derive(Debug)]
/// struct User {
///    email: &'static str,
///    name: &'static str,
/// }
///
/// let user = User {
///    email: "user@example.com",
///    name: "John Doe",
/// };
///
/// // NB the dotted name is not available in the format string
/// let span = logfire::span!("Span user info", user.email, user.name);
/// ```
///
/// [attributes]: https://opentelemetry.io/docs/concepts/signals/traces/#attributes
#[macro_export]
macro_rules! span {
    (parent: $parent:expr, level: $level:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(parent: $parent, $level, $format, $($($path).+ $(= $value)?),*)
    };
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(parent: $parent, $crate::__macros_impl::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
    (level: $level:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!($level, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!($crate::__macros_impl::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the ERROR level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! error {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, $crate::__macros_impl::Level::ERROR, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!($crate::__macros_impl::Level::ERROR, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the WARN level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! warn {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, $crate::__macros_impl::Level::WARN, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!($crate::__macros_impl::Level::WARN, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the INFO level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! info {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, $crate::__macros_impl::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!($crate::__macros_impl::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the DEBUG level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! debug {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, $crate::__macros_impl::Level::DEBUG, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!($crate::__macros_impl::Level::DEBUG, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the TRACE level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! trace {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, $crate::__macros_impl::Level::TRACE, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!($crate::__macros_impl::Level::TRACE, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Export a log message at the specified level.
///
/// # Syntax
///
/// The macro accepts the following syntax:
///
/// ```rust
/// # let value = 42;
/// logfire::log!(
///    parent: tracing::Span::current(), // optional, tracing::Span
///    tracing::Level::INFO,             // required, see `info!` and variants for convenience
///    "format string",                  // required, must be a format string accepted by `format!()`
///    attr = value,                     // optional, attribute key and value pair
///    // ..                             // as many additional attr = value pairs as desired
/// );
/// ```
///
/// ## Attributes
///
/// The `attr = value` pairs are captured as [attributes] on the span.
///
/// `dotted.name = value` is also supported (and encouraged by Opentelemetry) to namespace
/// attributes. However, dotted names are not supported in Rust format strings.
//
/// ## Formatting
///
/// The format string only accepts arguments by name. These can either be explicitly passed
/// to `span!` as an attribute or captured from the surrounding scope by the macro. If captured
/// from the surrounding scope, they will only be used as a format string and not exported
/// as attributes.
///
/// # Examples
///
/// ```rust,no_run
/// use tracing::Level;
///
/// // Root span
/// let root_span = logfire::span!("Root span");
///
/// // Log with attributes x and y
/// logfire::log!(parent: &root_span, Level::INFO, "Child log", x = 42, y = "hello");
/// // or
/// logfire::info!(parent: &root_span, "Child log", x = 42, y = "hello");
///
/// // Typically a span will be "entered" to set the parent implicitly
/// root_span.in_scope(|| {
///     // This log will be a child of root_span
///     logfire::log!(Level::INFO, "Nested log", x = 42, y = "hello");
///     // or
///     logfire::info!("Nested log", x = 42, y = "hello");
///
///     // Debug-level child log
///     logfire::log!(Level::DEBUG, "Debugging", x = 42, y = "hello");
///     // or
///     logfire::debug!("Debugging", x = 42, y = "hello");
/// });
///
/// // With x included in the formatted message but not as an attribute
/// let x = 42;
/// logfire::log!(Level::INFO, "Log with x = {x}, y = {y}", y = "hello");
/// // or
/// logfire::info!("Log with x = {x}, y = {y}", y = "hello");
///
/// // Attributes can either be a single name or a dotted.name
/// // `dotted.name` is not available in the format string.
/// logfire::log!(Level::INFO, "Log with x = {x}, y = {y}", y = "hello", foo.bar = 42);
/// // or
/// logfire::info!("Log with x = {x}, y = {y}", y = "hello", foo.bar = 42);
///
/// // If just an identifier is used without `= value`, it will be captured as an attribute
/// let foo = "bar";
/// logfire::log!(Level::INFO, "Log with foo = {foo}", foo);
/// // or
/// logfire::info!("Log with foo = {foo}", foo);
///
/// // This identifier shorthand also works for struct fields
/// #[derive(Debug)]
/// struct User {
///    email: &'static str,
///    name: &'static str,
/// }
///
/// let user = User {
///    email: "user@example.com",
///    name: "John Doe",
/// };
///
/// // NB the dotted name is not available in the format string
/// logfire::log!(Level::INFO, "Log user info", user.email, user.name);
/// // or
/// logfire::info!("Log user info", user.email, user.name);
/// ```
///
/// [attributes]: https://opentelemetry.io/docs/concepts/signals/traces/#attributes
#[macro_export]
macro_rules! log {
    (parent: $parent:expr, $level:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::log!(parent: $parent, $level, $format, $($($path).+ $(= $value)?),*)
    };
    ($level:expr, $format:expr  $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::log!(parent: $crate::__macros_impl::Span::current(), $level, $format, $($($path).+ $(= $value)?),*)
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{AdvancedOptions, ConsoleOptions, Target},
        logfire::LocalLogfireGuard,
    };
    use insta::assert_debug_snapshot;
    use opentelemetry::logs::AnyValue;
    use opentelemetry_sdk::logs::{InMemoryLogExporter, SimpleLogProcessor};
    use std::sync::{Arc, Mutex};
    use tracing::Level;

    fn get_output_guard(level: Level) -> (Arc<Mutex<Vec<u8>>>, LocalLogfireGuard) {
        let output = Arc::new(Mutex::new(Vec::new()));
        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(level)
        };

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .with_default_level_filter(tracing::level_filters::LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire);

        (output, guard)
    }

    #[test]
    fn test_error_macro() {
        let (output, guard) = get_output_guard(Level::ERROR);

        crate::error!("Test error message");
        crate::error!("Test error with value", field = 42i64);

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test error message"));
        assert!(output.contains("Test error with value"));
        assert!(output.contains("field") && output.contains("42"));
    }

    #[test]
    fn test_warn_macro() {
        let (output, guard) = get_output_guard(Level::WARN);

        crate::warn!("Test warn message");
        crate::warn!("Test warn with value", field = "test");

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test warn message"));
        assert!(output.contains("Test warn with value"));
        assert!(output.contains("field") && output.contains("test"));
    }

    #[test]
    fn test_info_macro() {
        let (output, guard) = get_output_guard(Level::INFO);

        crate::info!("Test info message");
        crate::info!("Test info with value", field = true);

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test info message"));
        assert!(output.contains("Test info with value"));
        assert!(output.contains("field") && output.contains("true"));
    }

    #[test]
    fn test_debug_macro() {
        let (output, guard) = get_output_guard(Level::DEBUG);

        crate::debug!("Test debug message");
        crate::debug!("Test debug with value", field = 3.14);

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test debug message"));
        assert!(output.contains("Test debug with value"));
        assert!(output.contains("field") && output.contains("3.14"));
    }

    #[test]
    fn test_trace_macro() {
        let (output, guard) = get_output_guard(Level::TRACE);

        crate::trace!("Test trace message");
        crate::trace!("Test trace with value", field = "debug_info");

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test trace message"));
        assert!(output.contains("Test trace with value"));
        assert!(output.contains("field") && output.contains("debug_info"));
    }

    #[test]
    fn test_log_macro_with_explicit_level() {
        let (output, guard) = get_output_guard(Level::INFO);

        crate::log!(Level::INFO, "Test log message");
        crate::log!(Level::INFO, "Test log with value", field = "explicit");

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test log message"));
        assert!(output.contains("Test log with value"));
        assert!(output.contains("field") && output.contains("explicit"));
    }

    #[test]
    fn test_macros_with_parent_span() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(Level::INFO)
        };

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .with_default_level_filter(tracing::level_filters::LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire.clone());

        let parent_span = crate::span!("parent span");
        crate::info!(parent: &parent_span, "Test info with parent");
        crate::error!(parent: &parent_span, "Test error with parent", field = "parent_test");

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test info with parent"));
        assert!(output.contains("Test error with parent"));
        assert!(output.contains("field") && output.contains("parent_test"));
    }

    #[test]
    fn test_rust_primitive_types_conversion_in_log() {
        let log_exporter = InMemoryLogExporter::default();

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_advanced_options(
                AdvancedOptions::default()
                    .with_log_processor(SimpleLogProcessor::new(log_exporter.clone())),
            )
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire);

        // i64::MAX =                9_223_372_036_854_775_807
        // so we test overflow with 10_000_000_000_000_000_000

        crate::info!("u8: {value}", value = 1u8);
        crate::info!("u16: {value}", value = 2u16);
        crate::info!("u32: {value}", value = 3u32);
        crate::info!("u64: {value}", value = 4u64);
        crate::info!(
            "u64 overflows i64: {value}",
            value = 10_000_000_000_000_000_001u64
        );
        crate::info!("u128: {value}", value = 5u128);
        crate::info!(
            "u128 overflows i64: {value}",
            value = 10_000_000_000_000_000_002u128
        );
        crate::info!("usize: {value}", value = 6usize);
        crate::info!(
            "usize overflows i64: {value}",
            value = 10_000_000_000_000_000_003u128
        );
        crate::info!("usize: {value}", value = 6usize);
        crate::info!("i8: {value}", value = 7i8);
        crate::info!("i16: {value}", value = 8i16);
        crate::info!("i32: {value}", value = 9i32);
        crate::info!("i64: {value}", value = 10i64);
        crate::info!("i128: {value}", value = 10i128);
        crate::info!(
            "i128 overflows i64: {value}",
            value = 10_000_000_000_000_000_004u64
        );
        crate::info!("isize: {value}", value = 11isize);
        crate::info!(
            "isize overflows i64: {value}",
            value = 10_000_000_000_000_000_005u64
        );
        crate::info!("f32: {value}", value = 1.2f32);
        crate::info!("f64: {value}", value = 3.4f64);
        crate::info!("bool: {value}", value = true);
        crate::info!("char: {value:?}", value = 'a');

        // repeat with optional attributes
        crate::info!("optional u8: {value:?}", value = Some(1u8));
        crate::info!("optional u16: {value:?}", value = Some(2u16));
        crate::info!("optional u32: {value:?}", value = Some(3u32));
        crate::info!("optional u64: {value:?}", value = Some(4u64));
        crate::info!(
            "optional u64 overflows i64: {value:?}",
            value = Some(10_000_000_000_000_000_001u64)
        );
        crate::info!("optional u128: {value:?}", value = Some(5u128));
        crate::info!(
            "optional u128 overflows i64: {value:?}",
            value = Some(10_000_000_000_000_000_002u128)
        );
        crate::info!("optional usize: {value:?}", value = Some(6usize));
        crate::info!(
            "optional usize overflows i64: {value:?}",
            value = Some(10_000_000_000_000_000_003usize)
        );
        crate::info!("optional usize: {value:?}", value = Some(6usize));
        crate::info!("optional i8: {value:?}", value = Some(7i8));
        crate::info!("optional i16: {value:?}", value = Some(8i16));
        crate::info!("optional i32: {value:?}", value = Some(9i32));
        crate::info!("optional i64: {value:?}", value = Some(10i64));
        crate::info!("optional i128: {value:?}", value = Some(10i128));
        crate::info!(
            "optional i128 overflows i64: {value:?}",
            value = Some(10_000_000_000_000_000_004i128)
        );
        crate::info!("optional isize: {value:?}", value = Some(11isize));
        crate::info!("optional f32: {value:?}", value = Some(1.2f32));
        crate::info!("optional f64: {value:?}", value = Some(3.4f64));
        crate::info!("optional bool: {value:?}", value = Some(true));
        crate::info!("optional char: {value:?}", value = Some('a'));

        // and without type hints for literal sizes
        crate::info!("integer: {value}", value = 12);
        crate::info!("float: {value}", value = 5.6);
        crate::info!("optional integer: {value:?}", value = Some(12));
        crate::info!("optional float: {value:?}", value = Some(5.6));

        guard.shutdown().unwrap();

        let logs = log_exporter.get_emitted_logs().unwrap();

        let messages_and_value = logs
            .iter()
            .map(|log| {
                let AnyValue::String(message) = log.record.body().unwrap() else {
                    panic!("Expected String body");
                };
                let value = log
                    .record
                    .attributes_iter()
                    .find(|(k, _)| k.as_str() == "value")
                    .map(|(_, v)| v.clone())
                    .unwrap();
                (message.as_str(), format!("{value:?}"))
            })
            .collect::<Vec<_>>();

        assert_debug_snapshot!(messages_and_value, @r#"
        [
            (
                "u8: 1",
                "Int(1)",
            ),
            (
                "u16: 2",
                "Int(2)",
            ),
            (
                "u32: 3",
                "Int(3)",
            ),
            (
                "u64: 4",
                "Int(4)",
            ),
            (
                "u64 overflows i64: 10000000000000000001",
                "String(Owned(\"10000000000000000001\"))",
            ),
            (
                "u128: 5",
                "Int(5)",
            ),
            (
                "u128 overflows i64: 10000000000000000002",
                "String(Owned(\"10000000000000000002\"))",
            ),
            (
                "usize: 6",
                "Int(6)",
            ),
            (
                "usize overflows i64: 10000000000000000003",
                "String(Owned(\"10000000000000000003\"))",
            ),
            (
                "usize: 6",
                "Int(6)",
            ),
            (
                "i8: 7",
                "Int(7)",
            ),
            (
                "i16: 8",
                "Int(8)",
            ),
            (
                "i32: 9",
                "Int(9)",
            ),
            (
                "i64: 10",
                "Int(10)",
            ),
            (
                "i128: 10",
                "Int(10)",
            ),
            (
                "i128 overflows i64: 10000000000000000004",
                "String(Owned(\"10000000000000000004\"))",
            ),
            (
                "isize: 11",
                "Int(11)",
            ),
            (
                "isize overflows i64: 10000000000000000005",
                "String(Owned(\"10000000000000000005\"))",
            ),
            (
                "f32: 1.2",
                "Double(1.2000000476837158)",
            ),
            (
                "f64: 3.4",
                "Double(3.4)",
            ),
            (
                "bool: true",
                "Boolean(true)",
            ),
            (
                "char: 'a'",
                "String(Owned(\"a\"))",
            ),
            (
                "optional u8: Some(1)",
                "Int(1)",
            ),
            (
                "optional u16: Some(2)",
                "Int(2)",
            ),
            (
                "optional u32: Some(3)",
                "Int(3)",
            ),
            (
                "optional u64: Some(4)",
                "Int(4)",
            ),
            (
                "optional u64 overflows i64: Some(10000000000000000001)",
                "String(Owned(\"10000000000000000001\"))",
            ),
            (
                "optional u128: Some(5)",
                "Int(5)",
            ),
            (
                "optional u128 overflows i64: Some(10000000000000000002)",
                "String(Owned(\"10000000000000000002\"))",
            ),
            (
                "optional usize: Some(6)",
                "Int(6)",
            ),
            (
                "optional usize overflows i64: Some(10000000000000000003)",
                "String(Owned(\"10000000000000000003\"))",
            ),
            (
                "optional usize: Some(6)",
                "Int(6)",
            ),
            (
                "optional i8: Some(7)",
                "Int(7)",
            ),
            (
                "optional i16: Some(8)",
                "Int(8)",
            ),
            (
                "optional i32: Some(9)",
                "Int(9)",
            ),
            (
                "optional i64: Some(10)",
                "Int(10)",
            ),
            (
                "optional i128: Some(10)",
                "Int(10)",
            ),
            (
                "optional i128 overflows i64: Some(10000000000000000004)",
                "String(Owned(\"10000000000000000004\"))",
            ),
            (
                "optional isize: Some(11)",
                "Int(11)",
            ),
            (
                "optional f32: Some(1.2)",
                "Double(1.2000000476837158)",
            ),
            (
                "optional f64: Some(3.4)",
                "Double(3.4)",
            ),
            (
                "optional bool: Some(true)",
                "Boolean(true)",
            ),
            (
                "optional char: Some('a')",
                "String(Owned(\"a\"))",
            ),
            (
                "integer: 12",
                "Int(12)",
            ),
            (
                "float: 5.6",
                "Double(5.599999904632568)",
            ),
            (
                "optional integer: Some(12)",
                "Int(12)",
            ),
            (
                "optional float: Some(5.6)",
                "Double(5.599999904632568)",
            ),
        ]
        "#);
    }
}
