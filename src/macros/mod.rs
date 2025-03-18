#[doc(hidden)]
#[path = "impl_.rs"]
pub mod __macros_impl;

/// Create a new [`Span`][tracing::Span]. This macro is a simplified version of `tracing::span!` which accepts
/// a smaller set of possible arguments and syntaxes.
///
/// In exchange, the macro automatically adds metadata which leads the span and its fields
/// to present nicely in the Logfire UI.
///
/// # Syntax
///
/// The macro accepts the following syntax:
///
/// ```rust,ignore
/// span!(
///    parent: tracing::Span, // optional, can emit this
///    level: tracing::Level, // optional
///    "format string",       // required, must be a format string accepted by `format!()`
///    arg1 = value1,         // optional, can be repeated
///    ..                     // as many additional arg = value pairs as desired
/// )
/// ```
///
/// The format string only accepts arguments by name.
///
/// These can be automatically captured from the surrounding scope by the macro,
/// in which case they will render in the message but will not be.
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
/// ```
#[macro_export]
macro_rules! span {
    (parent: $parent:expr, level: $level:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(parent: $parent, $level, $format, $($arg = $value),*)
    };
    (parent: $parent:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(parent: $parent, tracing::Level::INFO, $format, $($arg = $value),*)
    };
    (level: $level:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__macros_impl::tracing_span!($level, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(tracing::Level::INFO, $format, $($arg = $value),*)
    };
}

/// Emit a log at the ERROR level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! error {
    (parent: $parent:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::ERROR, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(tracing::Level::ERROR, $format, $($arg = $value),*)
    };
}

/// Emit a log at the WARN level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! warn {
    (parent: $parent:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::WARN, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(tracing::Level::WARN, $format, $($arg = $value),*)
    };
}

/// Emit a log at the INFO level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! info {
    (parent: $parent:expr, $format:expr $(,$arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::INFO, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(tracing::Level::INFO, $format, $($arg = $value),*)
    };
}

/// Emit a log at the ERROR level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! debug {
    (parent: $parent:expr, $format:expr $(,$arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::DEBUG, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::log!(tracing::Level::DEBUG, $format, $($arg = $value),*)
    };
}

/// Export a log message at the specified level.
///
/// # Syntax
///
/// The macro accepts the following syntax:
///
/// ```rust,ignore
/// log!(
///    parent: tracing::Span, // optional, can emit this
///    tracing::Level,        // required, see `info!` and variants for convenience
///    "format string",       // required, must be a format string accepted by `format!()`
///    arg1 = value1,         // optional, can be repeated
///    ..                     // as many additional arg = value pairs as desired
/// )
/// ```
///
/// The format string only accepts arguments by name.
///
/// These can be automatically captured from the surrounding scope by the macro,
/// in which case they will render in the message but will not be.
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
/// ```
#[macro_export]
macro_rules! log {
    (parent: $parent:expr, $level:expr, $format:expr, $($($arg:ident = $value:expr),+)?) => {{
        if tracing::span_enabled!($level) {
            // bind args early to avoid multiple evaluation
            $($(let $arg = $value;)*)?
            $crate::__macros_impl::export_log_span(
                $format,
                $parent,
                format!($format),
                $level,
                $crate::__json_schema!($($($arg),+)?),
                file!(),
                line!(),
                module_path!(),
                [
                    $($($crate::__macros_impl::LogfireValue::new(stringify!($arg), $crate::__macros_impl::converter(&$arg).convert_value($arg)),)*)?
                ]
            );
        }
    }};
    ($level:expr, $format:expr, $($($arg:ident = $value:expr),+)?) => {{
        if tracing::span_enabled!($level) {
            // bind args early to avoid multiple evaluation
            $($(let $arg = $value;)*)?
            $crate::__macros_impl::export_log_span(
                $format,
                &tracing::Span::current(),
                format!($format),
                $level,
                $crate::__json_schema!($($($arg),+)?),
                file!(),
                line!(),
                module_path!(),
                [
                    $($($crate::__macros_impl::LogfireValue::new(stringify!($arg), $crate::__macros_impl::converter(&$arg).convert_value($arg)),)*)?
                ]
            );
        }
    }};
}
