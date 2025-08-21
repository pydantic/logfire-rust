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
    use crate::config::{ConsoleOptions, Target};
    use std::sync::{Arc, Mutex};
    use tracing::Level;

    #[test]
    fn test_error_macro() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(Level::ERROR)
        };

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .with_default_level_filter(tracing::level_filters::LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire);

        crate::error!("Test error message");
        crate::error!("Test error with value", field = 42);

        guard.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();

        assert!(output.contains("Test error message"));
        assert!(output.contains("Test error with value"));
        assert!(output.contains("field") && output.contains("42"));
    }

    #[test]
    fn test_warn_macro() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(Level::WARN)
        };

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .with_default_level_filter(tracing::level_filters::LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire);

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

        let guard = crate::set_local_logfire(logfire);

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
        let output = Arc::new(Mutex::new(Vec::new()));
        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(Level::TRACE)
        };

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .with_default_level_filter(tracing::level_filters::LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire);

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
        let output = Arc::new(Mutex::new(Vec::new()));
        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(Level::TRACE)
        };

        let logfire = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options))
            .with_default_level_filter(tracing::level_filters::LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(logfire);

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

        let guard = crate::set_local_logfire(logfire);

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
}
