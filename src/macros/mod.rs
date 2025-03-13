#[doc(hidden)]
#[path = "impl_.rs"]
pub mod __macros_impl;

/// Create a new span.
///
/// The return type of this macro is a [`tracing::Span`], which can be used to enter the span
/// or to pass it to `tracing::Instrument` methods.
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
/// See the [`log!`] macro for more details.
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
/// See the [`log!`] macro for more details.
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
/// See the [`log!`] macro for more details.
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
/// See the [`log!`] macro for more details.
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
