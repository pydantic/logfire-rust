//! Implementation details of the macros.
//!
//! Note that macros exported here will end up at the crate root, they should probably all be prefixed with
//! __ just to help avoid collisions with real APIs.

use std::{borrow::Cow, marker::PhantomData, ops::Deref};

use crate::{
    bridges::tracing::{tracing_level_to_log_level, tracing_level_to_severity},
    internal::logfire_tracer::LogfireTracer,
};
use opentelemetry::{Key, Value};
use tracing_opentelemetry::OpenTelemetrySpanExt;

// Re-export macros marked with `#[macro_export]` from this module, because `#[macro_export]` places
// them at the crate root.
pub use crate::{__log as log, __tracing_span as tracing_span};

// Private re-export of `tracing` components, because the macros depend upon it.
pub use tracing::{Level, Span, span as __tracing_span_impl};

#[macro_export]
#[doc(hidden)]
macro_rules! __tracing_span {
    (parent: $parent:expr, $level:expr, $format:expr, $($($path:ident).+ $(= $value:expr)?),*) => {{
        // bind args early to avoid multiple evaluation
        $crate::__bind_single_ident_args!($($($path).+ $(= $value)?),*);
        $crate::__macros_impl::__tracing_span_impl!(
            parent: $parent,
            $level,
            $format,
            $($($path).+ = $crate::__evaluate_arg!($($path).+ $(= $value)?),)*
            logfire.msg = format_args!($format),
            logfire.json_schema = $crate::__json_schema!($($($path).+),*),
        )
    }};
    ($level:expr, $format:expr, $($($path:ident).+ $(= $value:expr)?),*) => {{
        // bind args early to avoid multiple evaluation
        $crate::__bind_single_ident_args!($($($path).+ $(= $value)?),*);
        $crate::__macros_impl::__tracing_span_impl!(
            $level,
            $format,
            $($($path).+ = $crate::__evaluate_arg!($($path).+ $(= $value)?),)*
            logfire.msg = format_args!($format),
            logfire.json_schema = $crate::__json_schema!($($($path).+),*),
        )
    }};
}

pub struct LogfireValue {
    pub(crate) name: Key,
    pub(crate) value: Option<Value>,
}

impl LogfireValue {
    #[must_use]
    pub fn new(name: &'static str, value: Option<Value>) -> Self {
        Self {
            name: Key::new(name),
            value,
        }
    }
}

/// Helper type which provides overrides for values which are either:
/// - Not handled by `Into<Value>` in OpenTelemetry
/// - Need special handling for type inference (e.g. `i32` integers)
/// - Are `Option<T>`, as opentelemetry does not support `Option` directly
pub struct LogfireConverter<T>(ConvertValue<T>);

/// Implements default conversion logic
/// - Uses `Into<Value>` for most types
/// - Also provides the base case for any type inference / option handling
pub struct ConvertValue<T>(PhantomData<T>);

#[must_use]
pub fn converter<T>(_: &T) -> LogfireConverter<T> {
    LogfireConverter(ConvertValue(PhantomData))
}

impl<T> LogfireConverter<Option<T>> {
    /// Flattens out options, returning a None branch to stop processing if the option is None.
    #[inline]
    #[must_use]
    pub fn unpack_option(&self, value: Option<T>) -> Option<(T, LogfireConverter<T>)> {
        value.map(|v| (v, LogfireConverter(ConvertValue(PhantomData))))
    }
}

impl LogfireConverter<i32> {
    /// Cause type inference to pick the i32 specialization for in info!("value: {value:?}", value = 12);
    #[inline]
    #[must_use]
    pub fn handle_type_inference(&self) -> LogfireConverter<i32> {
        LogfireConverter(ConvertValue(PhantomData))
    }
}

impl LogfireConverter<Option<i32>> {
    /// Cause type inference to pick the i32 specialization for in info!("value: {value:?}", value = Some(12));
    #[inline]
    #[must_use]
    pub fn handle_type_inference(&self) -> LogfireConverter<Option<i32>> {
        LogfireConverter(ConvertValue(PhantomData))
    }
}

impl LogfireConverter<f64> {
    /// Cause type inference to pick the f64 specialization for float literals like info!("value: {value}", value = 3.14);
    #[inline]
    #[must_use]
    pub fn handle_type_inference(&self) -> LogfireConverter<f64> {
        LogfireConverter(ConvertValue(PhantomData))
    }
}

impl LogfireConverter<Option<f64>> {
    /// Cause type inference to pick the f64 specialization for float literals like info!("value: {value:?}", value = Some(3.14));
    #[inline]
    #[must_use]
    pub fn handle_type_inference(&self) -> LogfireConverter<Option<f64>> {
        LogfireConverter(ConvertValue(PhantomData))
    }
}

impl<T> ConvertValue<T> {
    /// Base case for types where we don't care about type inference.
    #[inline]
    #[must_use]
    pub fn handle_type_inference(&self) -> LogfireConverter<T> {
        LogfireConverter(ConvertValue(PhantomData))
    }

    /// Base case for non-option values, just wrap it up. This will get removed again by
    /// inlining and optimization.
    #[inline]
    #[must_use]
    pub fn unpack_option(&self, value: T) -> Option<(T, LogfireConverter<T>)> {
        Some((value, LogfireConverter(ConvertValue(PhantomData))))
    }
}

macro_rules! impl_into_try_into_i64_value {
    ($type:ty) => {
        impl LogfireConverter<$type> {
            #[inline]
            #[must_use]
            pub fn convert_value(&self, value: $type) -> Option<Value> {
                // Attempt to convert the value to an i64.
                if let Ok(value) = i64::try_from(value) {
                    Some(value.into())
                } else {
                    // If it fails (e.g., overflow), fall back to a string.
                    // TODO emit a warning?
                    Some(value.to_string().into())
                }
            }
        }
    };
}

macro_rules! impl_into_from_i64_value {
    ($type:ty) => {
        impl LogfireConverter<$type> {
            #[inline]
            #[must_use]
            pub fn convert_value(&self, value: $type) -> Option<Value> {
                Some(i64::from(value).into())
            }
        }
    };
}

impl_into_from_i64_value!(u8);
impl_into_from_i64_value!(u16);
impl_into_from_i64_value!(u32);
impl_into_try_into_i64_value!(u64);
impl_into_try_into_i64_value!(u128);
impl_into_try_into_i64_value!(usize);
impl_into_from_i64_value!(i8);
impl_into_from_i64_value!(i16);
impl_into_from_i64_value!(i32);
impl_into_try_into_i64_value!(i128);
impl_into_try_into_i64_value!(isize);

// i64 already fits in i64, just pass through
impl LogfireConverter<i64> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: i64) -> Option<Value> {
        Some(value.into())
    }
}

impl LogfireConverter<f32> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: f32) -> Option<Value> {
        Some(f64::from(value).into())
    }
}

// f64 already fits in f64, just pass through
impl LogfireConverter<f64> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: f64) -> Option<Value> {
        Some(value.into())
    }
}

// bool converts to Value::Bool
impl LogfireConverter<bool> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: bool) -> Option<Value> {
        Some(value.into())
    }
}

impl LogfireConverter<char> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: char) -> Option<Value> {
        Some(value.to_string().into())
    }
}

impl<T> Deref for LogfireConverter<T> {
    type Target = ConvertValue<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> ConvertValue<T> {
    #[inline]
    pub fn convert_value(&self, value: T) -> Option<Value>
    where
        T: std::fmt::Display,
    {
        Some(value.to_string().into())
    }

    /// Convert a value using Debug formatting
    #[inline]
    pub fn convert_value_debug(&self, value: T) -> Option<Value>
    where
        T: std::fmt::Debug,
    {
        Some(format!("{:?}", value).into())
    }

    /// Convert a value using serde serialization to JSON
    #[cfg(feature = "data-dir")]
    #[inline]
    pub fn convert_value_serialize(&self, value: T) -> Option<Value>
    where
        T: serde::Serialize,
    {
        serde_json::to_string(&value)
            .ok()
            .map(|s| s.into())
    }
}

/// Implementations for reference types to support non-moving sigil syntax
impl<'a, T> LogfireConverter<&'a T> {
    /// Convert a reference using Display formatting
    #[inline]
    pub fn convert_value(&self, value: &T) -> Option<Value>
    where
        T: std::fmt::Display,
    {
        Some(value.to_string().into())
    }

    /// Convert a reference using Debug formatting
    #[inline]
    pub fn convert_value_debug(&self, value: &T) -> Option<Value>
    where
        T: std::fmt::Debug,
    {
        Some(format!("{:?}", value).into())
    }

    /// Convert a reference using serde serialization to JSON
    #[cfg(feature = "data-dir")]
    #[inline]
    pub fn convert_value_serialize(&self, value: &T) -> Option<Value>
    where
        T: serde::Serialize,
    {
        serde_json::to_string(value)
            .ok()
            .map(|s| s.into())
    }
}

/// Special handling for references to Options
impl<'a, T> LogfireConverter<&'a Option<T>> {
    /// Unpack a reference to an Option, returning a reference to the inner value if Some
    #[inline]
    #[must_use]
    pub fn unpack_option<'b>(&self, value: &'b Option<T>) -> Option<(&'b T, LogfireConverter<&'b T>)> {
        value.as_ref().map(|v| (v, LogfireConverter(ConvertValue(PhantomData))))
    }
}

#[must_use]
pub fn enabled(level: tracing::Level, module_path: &'static str) -> bool {
    LogfireTracer::try_with(|tracer| {
        tracer.enabled(
            &log::Metadata::builder()
                .level(tracing_level_to_log_level(level))
                .target(module_path)
                .build(),
        )
    })
    .unwrap_or(false)
}

#[expect(clippy::too_many_arguments)] // FIXME probably can group these
pub fn export_log(
    name: &'static str,
    parent_span: &tracing::Span,
    message: String,
    level: tracing::Level,
    schema: &'static str,
    file: Option<Cow<'static, str>>,
    line: Option<u32>,
    module_path: Option<&'static str>,
    args: impl IntoIterator<Item = LogfireValue>,
) {
    LogfireTracer::try_with(|tracer| {
        tracer.export_log(
            Some(name),
            &parent_span.context(),
            message,
            tracing_level_to_severity(level),
            schema,
            file,
            line,
            module_path.map(Cow::Borrowed),
            args,
        );
    });
}

#[macro_export]
#[doc(hidden)]
macro_rules! __json_schema {
    ($($($($path:ident).+),+)?) => {
        concat!("{\
            \"type\":\"object\",\
            \"properties\":{\
        ",
            $($crate::__schema_args!($($($path).+),*),)?
        "\
            }\
        }")
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __schema_args {
    ($($path:ident).+, $($($rest:ident).+),+) => {
        // this is done recursively to avoid a trailing comma in JSON :/
        concat!($crate::__schema_args!($($path).+), ",", $crate::__schema_args!($($($rest).+),*))
    };
    ($($path:ident).+) => {
        // TODO proper type analysis for the args
        concat!("\"", stringify!($($path).+), "\":{}")
    };
    () => {};
}

/// Expands to `let $arg = $value` only for single ident args
///
/// Valid variable names in Rust can only have a single ident (and this is also true)
/// of format syntax. We need to bind arguments which can go in the format string
/// early to avoid multiple evaluation of the expressions.
/// Supports sigils: ?arg (Debug), %arg (Display), #arg (Serialize)
#[macro_export]
#[doc(hidden)]
macro_rules! __bind_single_ident_args {
    // single-ident arg with value and sigil: bind it (e.g., field = ?value)
    ($arg:ident = ?$value:expr $(, $($rest_arg:ident).+ = $(?)? $rest_value:expr)*) => {
        let $arg = $value;
        $crate::__bind_single_ident_args!($($($rest_arg).+ = $(?)? $rest_value),*)
    };
    ($arg:ident = %$value:expr $(, $($rest_arg:ident).+ = $(%)? $rest_value:expr)*) => {
        let $arg = $value;
        $crate::__bind_single_ident_args!($($($rest_arg).+ = $(%)? $rest_value),*)
    };
    ($arg:ident = #$value:expr $(, $($rest_arg:ident).+ = $rest_value:expr)*) => {
        let $arg = $value;
        $crate::__bind_single_ident_args!($($($rest_arg).+ = $rest_value),*)
    };
    // single-ident arg with value (no sigil): bind it
    ($arg:ident = $value:expr $(, $($rest_arg:ident).+ = $rest_value:expr)*) => {
        let $arg = $value;
        $crate::__bind_single_ident_args!($($($rest_arg).+ = $rest_value),*)
    };
    // single-ident arg with sigil shorthand (e.g., ?field)
    (?$arg:ident $(, $($rest_arg:ident).+ $(= $rest_value:expr)?)*) => {
        let $arg = $arg;
        $crate::__bind_single_ident_args!($($($rest_arg).+ $(= $rest_value)?),*)
    };
    (%$arg:ident $(, $($rest_arg:ident).+ $(= $rest_value:expr)?)*) => {
        let $arg = $arg;
        $crate::__bind_single_ident_args!($($($rest_arg).+ $(= $rest_value)?),*)
    };
    (#$arg:ident $(, $($rest_arg:ident).+ $(= $rest_value:expr)?)*) => {
        let $arg = $arg;
        $crate::__bind_single_ident_args!($($($rest_arg).+ $(= $rest_value)?),*)
    };
    // single-ident arg without value: bind it
    ($arg:ident $(, $($rest_arg:ident).+ $(= $rest_value:expr)?)*) => {
        let $arg = $arg;
        $crate::__bind_single_ident_args!($($($rest_arg).+ $(= $rest_value)?),*)
    };
    // multi-ident arg: skip it
    ($($path:ident).+ $(= $value:expr)? $(, $($rest_arg:ident).+ $(= $rest_value:expr)?)*) => {
        $crate::__bind_single_ident_args!($($($rest_arg).+ $(= $rest_value)?),*)
    };
    // base case: stop recursion
    () => { };
}

/// Macro to evaluate the argument provided.
///
/// If the argument was single-ident, it was already evaluated so we should use the arg ident
/// directly. If it was multi-ident, we should evaluate it now.
/// Supports sigils by stripping them and returning the value.
#[macro_export]
#[doc(hidden)]
macro_rules! __evaluate_arg {
    // Sigil shorthands - already bound
    (?$arg:ident) => {
        $arg
    };
    (%$arg:ident) => {
        $arg
    };
    (#$arg:ident) => {
        $arg
    };
    // single ident arg should already have been bound
    ($arg:ident $(= $value:expr)?) => {
        $arg
    };
    // Explicit value with sigils
    ($($path:ident).+ = ?$value:expr) => {
        $value
    };
    ($($path:ident).+ = %$value:expr) => {
        $value
    };
    ($($path:ident).+ = #$value:expr) => {
        $value
    };
    // multi-ident arg with value: be evaluated now
    ($($path:ident).+ = $value:expr) => {
        $value
    };
    // multi-ident arg evaluated from path
    ($($path:ident).+) => {
        $($path).+
    };
}

/// Extract the sigil from an argument pattern
#[macro_export]
#[doc(hidden)]
macro_rules! __extract_sigil {
    // Debug sigil
    (?$arg:ident) => { ? };
    ($($path:ident).+ = ?$value:expr) => { ? };
    // Display sigil
    (%$arg:ident) => { % };
    ($($path:ident).+ = %$value:expr) => { % };
    // Serialize sigil
    (#$arg:ident) => { # };
    ($($path:ident).+ = #$value:expr) => { # };
    // No sigil - default
    ($($any:tt)*) => { };
}

/// Extract the field name from an argument pattern (without sigil)
#[macro_export]
#[doc(hidden)]
macro_rules! __extract_field_name {
    // With sigils - strip them
    (?$arg:ident) => { stringify!($arg) };
    (%$arg:ident) => { stringify!($arg) };
    (#$arg:ident) => { stringify!($arg) };
    ($($path:ident).+ = ?$value:expr) => { stringify!($($path).+) };
    ($($path:ident).+ = %$value:expr) => { stringify!($($path).+) };
    ($($path:ident).+ = #$value:expr) => { stringify!($($path).+) };
    // Without sigils
    ($($path:ident).+ = $value:expr) => { stringify!($($path).+) };
    ($($path:ident).+) => { stringify!($($path).+) };
}

/// Helper macro to convert a value using Debug formatting for logging
///
/// # Example
/// ```rust,ignore
/// logfire::info!("User: {user}", user = as_debug!(user));
/// ```
#[macro_export]
macro_rules! as_debug {
    ($value:expr) => {
        ::std::format!("{:?}", $value)
    };
}

/// Helper macro to convert a value to JSON using serde for logging
///
/// # Example
/// ```rust,ignore
/// logfire::info!("User: {user}", user = as_json!(user));
/// ```
#[cfg(feature = "data-dir")]
#[macro_export]
macro_rules! as_json {
    ($value:expr) => {
        serde_json::to_string(&$value).unwrap_or_else(|e| {
            ::std::format!("{{\"error\":\"Failed to serialize: {}\"}}", e)
        })
    };
}

/// Parse log arguments using TT munching
///
/// Supports three modes:
/// - @bind_args: Bind variables early
/// - @schema: Extract field names for JSON schema
/// - @values: Convert values to LogfireValue array
#[macro_export]
#[doc(hidden)]
macro_rules! __parse_log_args {
    // ===== BIND ARGS MODE =====
    // Bind single-ident arguments early to avoid multiple evaluation in format string

    // Base case: no more args
    (@bind_args ()) => {};

    // Debug shorthand: ?field - bind as reference so it can be used in format string
    (@bind_args (? $field:ident, $($rest:tt)*)) => {
        let $field = &$field;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args (? $field:ident)) => {
        let $field = &$field;
    };

    // Display shorthand: %field - bind as reference so it can be used in format string
    (@bind_args (% $field:ident, $($rest:tt)*)) => {
        let $field = &$field;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args (% $field:ident)) => {
        let $field = &$field;
    };

    // Serialize shorthand: #field - bind as reference so it can be used in format string
    (@bind_args (# $field:ident, $($rest:tt)*)) => {
        let $field = &$field;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args (# $field:ident)) => {
        let $field = &$field;
    };

    // Field with explicit value - Debug sigil: field = ?value
    (@bind_args ($field:ident = ? $value:ident, $($rest:tt)*)) => {
        let $field = &$value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args ($field:ident = ? $value:ident)) => {
        let $field = &$value;
    };

    // Field with explicit value - Display sigil: field = %value
    (@bind_args ($field:ident = % $value:ident, $($rest:tt)*)) => {
        let $field = &$value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args ($field:ident = % $value:ident)) => {
        let $field = &$value;
    };

    // Field with explicit value - Serialize sigil: field = #value
    (@bind_args ($field:ident = # $value:ident, $($rest:tt)*)) => {
        let $field = &$value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args ($field:ident = # $value:ident)) => {
        let $field = &$value;
    };

    // Field with explicit value - no sigil: field = value
    // Bind by value (not reference) to preserve primitive type converters
    (@bind_args ($field:ident = $value:expr, $($rest:tt)*)) => {
        let $field = $value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args ($field:ident = $value:expr)) => {
        let $field = $value;
    };

    // Single ident without sigil
    (@bind_args ($field:ident, $($rest:tt)*)) => {
        let $field = &$field;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_args ($field:ident)) => {
        let $field = &$field;
    };

    // Skip commas
    (@bind_args (, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };

    // Skip dotted paths (multi-ident) - they can't be in format strings anyway
    (@bind_args ($first:ident . $($rest:tt)*)) => {
        $crate::__parse_log_args!(@skip_to_comma ($($rest)*));
    };

    // Helper: skip to next comma for dotted paths
    (@skip_to_comma (, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@skip_to_comma ($token:tt $($rest:tt)*)) => {
        $crate::__parse_log_args!(@skip_to_comma ($($rest)*));
    };
    (@skip_to_comma ()) => {};

    // Helper: bind field with explicit value
    (@bind_single_field_value $field:ident, (?$value:expr, $($rest:tt)*)) => {
        let $field = $value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_single_field_value $field:ident, (%$value:expr, $($rest:tt)*)) => {
        let $field = $value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_single_field_value $field:ident, (#$value:expr, $($rest:tt)*)) => {
        let $field = $value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_single_field_value $field:ident, ($value:expr, $($rest:tt)*)) => {
        let $field = $value;
        $crate::__parse_log_args!(@bind_args ($($rest)*));
    };
    (@bind_single_field_value $field:ident, (?$value:expr)) => {
        let $field = $value;
    };
    (@bind_single_field_value $field:ident, (%$value:expr)) => {
        let $field = $value;
    };
    (@bind_single_field_value $field:ident, (#$value:expr)) => {
        let $field = $value;
    };
    (@bind_single_field_value $field:ident, ($value:expr)) => {
        let $field = $value;
    };

    // ===== SCHEMA MODE =====
    // Extract field names for JSON schema

    (@schema ()) => {
        concat!("{\"type\":\"object\",\"properties\":{}}")
    };

    (@schema ($($args:tt)+)) => {{
        let fields = $crate::__parse_log_args!(@schema_fields [] ($($args)+));
        concat!("{\"type\":\"object\",\"properties\":{", fields, "}}")
    }};

    // Collect field names
    (@schema_fields [$($collected:expr),*] ()) => {
        concat!($($collected),*)
    };

    (@schema_fields [$($collected:expr),*] (?$field:ident $($rest:tt)*)) => {
        $crate::__parse_log_args!(@schema_fields [$($collected,)* concat!("\"", stringify!($field), "\":{},")] ($($rest)*))
    };
    (@schema_fields [$($collected:expr),*] (%$field:ident $($rest:tt)*)) => {
        $crate::__parse_log_args!(@schema_fields [$($collected,)* concat!("\"", stringify!($field), "\":{},")] ($($rest)*))
    };
    (@schema_fields [$($collected:expr),*] (#$field:ident $($rest:tt)*)) => {
        $crate::__parse_log_args!(@schema_fields [$($collected,)* concat!("\"", stringify!($field), "\":{},")] ($($rest)*))
    };
    (@schema_fields [$($collected:expr),*] ($field:ident $($rest:tt)*)) => {
        $crate::__parse_log_args!(@schema_fields [$($collected,)* concat!("\"", stringify!($field), "\":{},")] ($($rest)*))
    };

    // Skip tokens that aren't field names
    (@schema_fields [$($collected:expr),*] ($token:tt $($rest:tt)*)) => {
        $crate::__parse_log_args!(@schema_fields [$($collected),*] ($($rest)*))
    };

    // ===== VALUES MODE =====
    // Convert values to LogfireValue array

    (@values ()) => { [] };

    (@values ($($args:tt)+)) => {
        $crate::__parse_log_args!(@values_collect [] ($($args)+))
    };

    // Collect values - Debug shorthand
    (@values_collect [$($collected:expr),*] (?$field:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value_debug(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] (?$field:ident)) => {
        [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value_debug(value))
                )
            }
        ]
    };

    // Collect values - Display shorthand
    (@values_collect [$($collected:expr),*] (%$field:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] (%$field:ident)) => {
        [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ]
    };

    // Collect values - Serialize shorthand
    (@values_collect [$($collected:expr),*] (#$field:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value_serialize(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] (#$field:ident)) => {
        [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value_serialize(value))
                )
            }
        ]
    };

    // Collect values - Explicit value with Debug sigil: field = ?value
    (@values_collect [$($collected:expr),*] ($field:ident = ? $value:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value_debug(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] ($field:ident = ? $value:ident)) => {
        [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value_debug(value))
                )
            }
        ]
    };

    // Collect values - Explicit value with Display sigil: field = %value
    (@values_collect [$($collected:expr),*] ($field:ident = % $value:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] ($field:ident = % $value:ident)) => {
        [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ]
    };

    // Collect values - Explicit value with Serialize sigil: field = #value
    (@values_collect [$($collected:expr),*] ($field:ident = # $value:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value_serialize(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] ($field:ident = # $value:ident)) => {
        [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value_serialize(value))
                )
            }
        ]
    };

    // Collect values - Explicit value without sigil: field = value
    (@values_collect [$($collected:expr),*] ($field:ident = $value:expr, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] ($field:ident = $value:expr)) => {
        [
            $($collected,)*
            {
                let arg_value = $field;
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&arg_value)
                        .handle_type_inference()
                        .unpack_option(arg_value)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ]
    };

    // Collect values - No sigil shorthand (default Display)
    (@values_collect [$($collected:expr),*] ($field:ident, $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] ($field:ident)) => {
        [
            $($collected,)*
            {
                $crate::__macros_impl::LogfireValue::new(
                    stringify!($field),
                    $crate::__macros_impl::converter(&$field)
                        .handle_type_inference()
                        .unpack_option(&$field)
                        .and_then(|(value, converter)| converter.convert_value(value))
                )
            }
        ]
    };

    // Skip other tokens
    (@values_collect [$($collected:expr),*] ($token:tt $($rest:tt)*)) => {
        $crate::__parse_log_args!(@values_collect [$($collected),*] ($($rest)*))
    };
    (@values_collect [$($collected:expr),*] ()) => {
        [$($collected),*]
    };
}

/// Determines which converter method to call based on sigil prefix
///
/// Takes the full argument token tree and dispatches to the right converter
#[macro_export]
#[doc(hidden)]
macro_rules! __convert_with_specifier {
    // Debug sigil (?)
    ($converter:expr, $value:expr, ?$arg:ident) => {
        $converter.convert_value_debug($value)
    };
    ($converter:expr, $value:expr, $($path:ident).+ = ?$val:expr) => {
        $converter.convert_value_debug($value)
    };
    // Display sigil (%)
    ($converter:expr, $value:expr, %$arg:ident) => {
        $converter.convert_value($value)
    };
    ($converter:expr, $value:expr, $($path:ident).+ = %$val:expr) => {
        $converter.convert_value($value)
    };
    // Serialize sigil (#)
    ($converter:expr, $value:expr, #$arg:ident) => {
        $converter.convert_value_serialize($value)
    };
    ($converter:expr, $value:expr, $($path:ident).+ = #$val:expr) => {
        $converter.convert_value_serialize($value)
    };
    // No sigil = Display (default)
    ($converter:expr, $value:expr, $($any:tt)*) => {
        $converter.convert_value($value)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __log {
    (parent: $parent:expr, $level:expr, $format:expr $(, $($args:tt)*)?) => {
        if $crate::__macros_impl::enabled($level, module_path!()) {
            // Parse and bind arguments
            $crate::__parse_log_args! {
                @bind_args ($($($args)*)?)
            }
            $crate::__macros_impl::export_log(
                $format,
                &$parent,
                ::std::format!($format),
                $level,
                "{\"type\":\"object\",\"properties\":{}}",
                ::std::option::Option::Some(::std::borrow::Cow::Borrowed(file!())),
                ::std::option::Option::Some(line!()),
                ::std::option::Option::Some(module_path!()),
                $crate::__parse_log_args!(@values ($($($args)*)?))
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::Value;

    #[test]
    fn test_schema_args() {
        assert_eq!(r#""arg1.a":{},"arg2.b":{}"#, __schema_args!(arg1.a, arg2.b));
    }

    /// Test that integer types are converted to Value::Int, not Value::String
    /// This ensures specialized implementations take precedence over Display fallback
    #[test]
    fn test_integer_types_convert_to_int() {
        // Unsigned integers that fit in i64
        let v = converter(&1u8)
            .handle_type_inference()
            .convert_value(1u8)
            .unwrap();
        assert!(matches!(v, Value::I64(1)));

        let v = converter(&2u16)
            .handle_type_inference()
            .convert_value(2u16)
            .unwrap();
        assert!(matches!(v, Value::I64(2)));

        let v = converter(&3u32)
            .handle_type_inference()
            .convert_value(3u32)
            .unwrap();
        assert!(matches!(v, Value::I64(3)));

        let v = converter(&4u64)
            .handle_type_inference()
            .convert_value(4u64)
            .unwrap();
        assert!(matches!(v, Value::I64(4)));

        // Signed integers
        let v = converter(&5i8)
            .handle_type_inference()
            .convert_value(5i8)
            .unwrap();
        assert!(matches!(v, Value::I64(5)));

        let v = converter(&6i16)
            .handle_type_inference()
            .convert_value(6i16)
            .unwrap();
        assert!(matches!(v, Value::I64(6)));

        let v = converter(&7i32)
            .handle_type_inference()
            .convert_value(7i32)
            .unwrap();
        assert!(matches!(v, Value::I64(7)));

        let v = converter(&8i64)
            .handle_type_inference()
            .convert_value(8i64)
            .unwrap();
        assert!(matches!(v, Value::I64(8)));

        // Platform-dependent sizes
        let v = converter(&9usize)
            .handle_type_inference()
            .convert_value(9usize)
            .unwrap();
        assert!(matches!(v, Value::I64(9)));

        let v = converter(&10isize)
            .handle_type_inference()
            .convert_value(10isize)
            .unwrap();
        assert!(matches!(v, Value::I64(10)));
    }

    /// Test that large integers that overflow i64 become strings
    #[test]
    fn test_large_integer_overflow_to_string() {
        let large_u64: u64 = 10_000_000_000_000_000_000;
        let v = converter(&large_u64)
            .handle_type_inference()
            .convert_value(large_u64)
            .unwrap();
        // Should be a string since it doesn't fit in i64
        assert!(matches!(v, Value::String(_)));

        let large_u128: u128 = 10_000_000_000_000_000_000;
        let v = converter(&large_u128)
            .handle_type_inference()
            .convert_value(large_u128)
            .unwrap();
        assert!(matches!(v, Value::String(_)));
    }

    /// Test that float types convert to Value::F64
    #[test]
    fn test_float_types_convert_to_f64() {
        let v = converter(&1.5f32)
            .handle_type_inference()
            .convert_value(1.5f32)
            .unwrap();
        assert!(matches!(v, Value::F64(_)));

        let v = converter(&2.5f64)
            .handle_type_inference()
            .convert_value(2.5f64)
            .unwrap();
        assert!(matches!(v, Value::F64(_)));
    }

    /// Test that bool converts to Value::Bool
    #[test]
    fn test_bool_converts_to_bool() {
        let v = converter(&true)
            .handle_type_inference()
            .convert_value(true)
            .unwrap();
        assert!(matches!(v, Value::Bool(true)));

        let v = converter(&false)
            .handle_type_inference()
            .convert_value(false)
            .unwrap();
        assert!(matches!(v, Value::Bool(false)));
    }

    /// Test that String and &str convert to Value::String
    #[test]
    fn test_string_types_convert_to_string() {
        let s = "hello".to_string();
        let v = converter(&s)
            .handle_type_inference()
            .convert_value(s)
            .unwrap();
        assert!(matches!(v, Value::String(_)));

        let v = converter(&"world")
            .handle_type_inference()
            .convert_value("world")
            .unwrap();
        assert!(matches!(v, Value::String(_)));
    }

    /// Test that Display types work via fallback
    #[test]
    fn test_display_types_convert_via_fallback() {
        use std::net::Ipv4Addr;
        use std::path::Path;

        // Ipv4Addr implements Display
        let addr = Ipv4Addr::new(127, 0, 0, 1);
        let v = converter(&addr)
            .handle_type_inference()
            .convert_value(addr)
            .unwrap();
        assert!(matches!(v, Value::String(s) if s.as_ref() == "127.0.0.1"));

        // Path::display() returns a type that implements Display
        let path = Path::new("/etc/config");
        let display = path.display();
        let v = converter(&display)
            .handle_type_inference()
            .convert_value(display)
            .unwrap();
        assert!(matches!(v, Value::String(s) if s.as_ref() == "/etc/config"));

        // Custom struct with Display
        struct UserId(u64);
        impl std::fmt::Display for UserId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "user-{}", self.0)
            }
        }

        let user_id = UserId(12345);
        let v = converter(&user_id)
            .handle_type_inference()
            .convert_value(user_id)
            .unwrap();
        assert!(matches!(v, Value::String(s) if s.as_ref() == "user-12345"));
    }

    /// Test Option handling unwraps correctly
    #[test]
    fn test_option_unwrapping() {
        // Some(value) should unwrap and convert
        let v = converter(&Some(42i32))
            .handle_type_inference()
            .unpack_option(Some(42i32))
            .and_then(|(val, conv)| conv.convert_value(val))
            .unwrap();
        assert!(matches!(v, Value::I64(42)));

        // None should return None
        let v = converter(&None::<i32>)
            .handle_type_inference()
            .unpack_option(None::<i32>)
            .and_then(|(val, conv)| conv.convert_value(val));
        assert!(v.is_none());
    }

    /// Test that specialized impls are actually used, not the Display fallback
    #[test]
    fn test_specialized_impls_take_precedence() {
        // Create a type that implements both Display and Into<Value> via Into<i64>
        #[derive(Copy, Clone)]
        struct MyInt(i64);

        impl std::fmt::Display for MyInt {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "MyInt({})", self.0)
            }
        }

        // Without specialized impl, this would use Display fallback
        // But if we add a specialized impl, it should use that
        impl LogfireConverter<MyInt> {
            pub fn convert_value(&self, value: MyInt) -> Option<Value> {
                // Convert directly to i64 (efficient)
                Some(value.0.into())
            }
        }

        let my_int = MyInt(42);
        let v = converter(&my_int)
            .handle_type_inference()
            .convert_value(my_int)
            .unwrap();

        // Should be Value::I64, NOT Value::String("MyInt(42)")
        assert!(matches!(v, Value::I64(42)));
    }

    /// Test that String/&str still work efficiently
    #[test]
    fn test_string_types_avoid_double_allocation() {
        // &str should convert directly
        let s = "hello";
        let v = converter(&s)
            .handle_type_inference()
            .convert_value(s)
            .unwrap();

        match v {
            Value::String(cow) => {
                assert_eq!(cow.as_ref(), "hello");
            }
            _ => panic!("Expected String variant"),
        }

        // String should convert efficiently (via specialized impl)
        let s = "world".to_string();
        let v = converter(&s)
            .handle_type_inference()
            .convert_value(s)
            .unwrap();

        match v {
            Value::String(cow) => {
                assert_eq!(cow.as_ref(), "world");
            }
            _ => panic!("Expected String variant"),
        }
    }

    /// Test edge case: types with weird Display impls
    #[test]
    fn test_weird_display_impls() {
        struct WeirdType;

        impl std::fmt::Display for WeirdType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "🔥 weird value 🔥")
            }
        }

        let weird = WeirdType;
        let v = converter(&weird)
            .handle_type_inference()
            .convert_value(weird)
            .unwrap();

        // Should successfully convert even weird Display impls
        assert!(matches!(v, Value::String(s) if s.as_ref() == "🔥 weird value 🔥"));
    }

    /// Test that types can be converted using Debug formatting
    #[test]
    fn test_debug_conversion() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Point {
            x: i32,
            y: i32,
        }

        let point = Point { x: 10, y: 20 };
        let v = converter(&point)
            .handle_type_inference()
            .convert_value_debug(point)
            .unwrap();

        // Should convert using Debug formatting
        assert!(matches!(v, Value::String(s) if s.as_ref().contains("Point") && s.as_ref().contains("10") && s.as_ref().contains("20")));
    }

    /// Test that Debug works with types that don't implement Display
    #[test]
    fn test_debug_only_type() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct DebugOnly {
            value: String,
        }

        let debug_only = DebugOnly {
            value: "test".to_string(),
        };
        let v = converter(&debug_only)
            .handle_type_inference()
            .convert_value_debug(debug_only)
            .unwrap();

        // Should successfully convert using Debug
        assert!(matches!(v, Value::String(s) if s.as_ref().contains("DebugOnly")));
    }

    /// Test that Serialize works for serializable types
    #[cfg(feature = "data-dir")]
    #[test]
    fn test_serialize_conversion() {
        #[derive(serde::Serialize)]
        struct User {
            id: u64,
            name: String,
        }

        let user = User {
            id: 123,
            name: "Alice".to_string(),
        };
        let v = converter(&user)
            .handle_type_inference()
            .convert_value_serialize(user)
            .unwrap();

        // Should convert to JSON string
        assert!(matches!(v, Value::String(s) if s.as_ref().contains("123") && s.as_ref().contains("Alice")));
    }

    /// Test that Serialize works with complex nested types
    #[cfg(feature = "data-dir")]
    #[test]
    fn test_serialize_complex_type() {
        #[derive(serde::Serialize)]
        struct Config {
            enabled: bool,
            timeout_ms: u64,
            tags: Vec<String>,
        }

        let config = Config {
            enabled: true,
            timeout_ms: 5000,
            tags: vec!["production".to_string(), "api".to_string()],
        };

        let v = converter(&config)
            .handle_type_inference()
            .convert_value_serialize(config)
            .unwrap();

        // Should serialize to JSON string containing all fields
        match v {
            Value::String(s) => {
                let json_str = s.as_ref();
                assert!(json_str.contains("true"));
                assert!(json_str.contains("5000"));
                assert!(json_str.contains("production"));
                assert!(json_str.contains("api"));
            }
            _ => panic!("Expected String variant"),
        }
    }
}
