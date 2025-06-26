//! Implementation details of the macros.
//!
//! Note that macros exported here will end up at the crate root, they should probably all be prefixed with
//! __ just to help avoid collisions with real APIs.

use std::{borrow::Cow, marker::PhantomData, ops::Deref, time::SystemTime};

use crate::{bridges::tracing::level_to_level_number, try_with_logfire_tracer};
use opentelemetry::{
    Array, Key, KeyValue, StringValue, Value, global::ObjectSafeSpan, trace::Tracer as _,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

// Re-export macros marked with `#[macro_export]` from this module, because `#[macro_export]` places
// them at the crate root.
pub use crate::{__log as log, __tracing_span as tracing_span};

#[macro_export]
#[doc(hidden)]
macro_rules! __tracing_span {
    (parent: $parent:expr, $level:expr, $format:expr, $($($path:ident).+ $(= $value:expr)?),*) => {{
        // bind args early to avoid multiple evaluation
        $crate::__bind_single_ident_args!($($($path).+ $(= $value)?),*);
        tracing::span!(
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
        tracing::span!(
            $level,
            $format,
            $($($path).+ = $crate::__evaluate_arg!($($path).+ $(= $value)?),)*
            logfire.msg = format_args!($format),
            logfire.json_schema = $crate::__json_schema!($($($path).+),*),
        )
    }};
}

pub struct LogfireValue {
    name: Key,
    value: Option<Value>,
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

// Helper structure which converts arguments which might be optional into
// otel arguments. Otel arguments are not allowed to be optional, so we
// have to lift this a layer.

pub struct FallbackToConvertValue<T>(PhantomData<T>);

pub struct TryConvertOption<T>(FallbackToConvertValue<T>);

pub struct LogfireConverter<T>(TryConvertOption<T>);

#[must_use]
pub fn converter<T>(_: &T) -> LogfireConverter<T> {
    LogfireConverter(TryConvertOption(FallbackToConvertValue(PhantomData)))
}

/// `usize` might exceed OTLP range of i64. The Python SDK handles this by converting
/// to a string for oversize values, we do the same.
impl LogfireConverter<usize> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: usize) -> Option<Value> {
        if let Ok(value) = i64::try_from(value) {
            Some(value.into())
        } else {
            // TODO emit a warning?
            Some(value.to_string().into())
        }
    }
}

/// Convenience to take ownership of borrow on String
impl LogfireConverter<&'_ String> {
    #[inline]
    #[must_use]
    pub fn convert_value(&self, value: &String) -> Option<Value> {
        Some(String::to_owned(value).into())
    }
}

impl<T> Deref for LogfireConverter<T> {
    type Target = TryConvertOption<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> TryConvertOption<Option<T>> {
    #[inline]
    pub fn convert_value(&self, value: Option<T>) -> Option<Value>
    where
        T: Into<Value>,
    {
        value.map(Into::into)
    }
}

impl<T> Deref for TryConvertOption<T> {
    type Target = FallbackToConvertValue<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> FallbackToConvertValue<T> {
    #[inline]
    pub fn convert_value(&self, value: T) -> Option<Value>
    where
        T: Into<Value>,
    {
        Some(value.into())
    }
}

#[expect(clippy::too_many_arguments)] // FIXME probably can group these
pub fn export_log_span(
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
    thread_local! {
        static THREAD_ID: i64 = {
            // thread ID doesn't expose inner value, so we have to parse it out :(
            // (tracing-opentelemetry does the same)
            // format is ThreadId(N)
            let s = format!("{:?}", std::thread::current().id());
            let data = s.split_at(9).1;
            let data = data.split_at(data.len() - 1).0;
            data.parse().expect("should always be a valid number")
        }
    }

    try_with_logfire_tracer(|tracer| {
        let mut null_args: Vec<StringValue> = Vec::new();

        let mut attributes: Vec<_> = args
            .into_iter()
            .filter_map(|arg| {
                if let Some(value) = arg.value {
                    Some(KeyValue::new(arg.name, value))
                } else {
                    // FIXME: otel should allow Key <=> StringValue to avoid copy
                    null_args.push(arg.name.as_str().to_owned().into());
                    None
                }
            })
            .chain([
                KeyValue::new("logfire.msg", message),
                KeyValue::new("logfire.level_num", level_to_level_number(level)),
                KeyValue::new("logfire.span_type", "log"),
                KeyValue::new("logfire.json_schema", schema),
                KeyValue::new("thread.id", THREAD_ID.with(|id| *id)),
            ])
            .chain(
                // if thread.name is available, add it
                std::thread::current()
                    .name()
                    .map(|name| KeyValue::new("thread.name", name.to_owned())),
            )
            .collect();

        if let Some(file) = file {
            attributes.push(KeyValue::new("code.filepath", file));
        }

        if let Some(line) = line {
            attributes.push(KeyValue::new("code.lineno", i64::from(line)));
        }

        if let Some(module_path) = module_path {
            attributes.push(KeyValue::new("code.namespace", module_path));
        }

        if !null_args.is_empty() {
            attributes.push(KeyValue::new(
                "logfire.null_args",
                Value::Array(Array::from(null_args)),
            ));
        }

        let ts = SystemTime::now();

        tracer
            .inner
            .span_builder(name)
            .with_attributes(attributes)
            .with_start_time(ts)
            // .with_end_time(ts) seems to not be respected, need to explicitly end as per below
            .start_with_context(&tracer.inner, &parent_span.context())
            .end_with_timestamp(ts);
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
#[macro_export]
#[doc(hidden)]
macro_rules! __bind_single_ident_args {
    // single-ident arg with value: bind it
    ($arg:ident = $value:expr $(, $($rest_arg:ident).+ = $rest_value:expr)*) => {
        let $arg = $value;
        $crate::__bind_single_ident_args!($($($rest_arg).+ = $rest_value),*)
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
#[macro_export]
#[doc(hidden)]
macro_rules! __evaluate_arg {
    // single ident arg should already have been bound
    ($arg:ident $(= $value:expr)?) => {
        $arg
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

#[macro_export]
#[doc(hidden)]
macro_rules! __log {
    (parent: $parent:expr, $level:expr, $format:expr, $($($path:ident).+ $(= $value:expr)?),*) => {
        if tracing::span_enabled!($level) {
            // bind single ident args early to allow them in the format string
            // without multiple evaluation
            $crate::__bind_single_ident_args!($($($path).+ $(= $value)?),*);
            $crate::__macros_impl::export_log_span(
                $format,
                $parent,
                format!($format),
                $level,
                $crate::__json_schema!($($($path).+),*),
                Some(::std::borrow::Cow::Borrowed(file!())),
                Some(line!()),
                Some(module_path!()),
                [
                    $({
                        let arg_value = $crate::__evaluate_arg!($($path).+ $(= $value)?);
                        $crate::__macros_impl::LogfireValue::new(
                            stringify!($($path).+),
                            $crate::__macros_impl::converter(&arg_value).convert_value(arg_value)
                        )
                    }),*
                ]
            );
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_schema_args() {
        assert_eq!(r#""arg1.a":{},"arg2.b":{}"#, __schema_args!(arg1.a, arg2.b));
    }
}
