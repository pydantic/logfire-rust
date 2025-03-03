use std::{marker::PhantomData, ops::Deref, time::SystemTime};

use crate::{bridges::tracing::level_to_level_number, try_with_logfire_tracer};
use opentelemetry::{
    Array, Key, KeyValue, StringValue, Value, global::ObjectSafeSpan, trace::Tracer as _,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[macro_export]
macro_rules! span {
    (parent: $parent:expr, level: $level:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__tracing_span!(parent: $parent, $level, $format, $($arg = $value),*)
    };
    (parent: $parent:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__tracing_span!(parent: $parent, tracing::Level::INFO, $format, $($arg = $value),*)
    };
    (level: $level:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__tracing_span!($level, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__tracing_span!(tracing::Level::INFO, $format, $($arg = $value),*)
    };
}

#[macro_export]
macro_rules! error {
    (parent: $parent:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(parent: $parent, tracing::Level::ERROR, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(tracing::Level::ERROR, $format, $($arg = $value),*)
    };
}

#[macro_export]
macro_rules! warn {
    (parent: $parent:expr, $format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(parent: $parent, tracing::Level::WARN, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(tracing::Level::WARN, $format, $($arg = $value),*)
    };
}

#[macro_export]
macro_rules! info {
    (parent: $parent:expr, $format:expr $(,$arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(parent: $parent, tracing::Level::INFO, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(tracing::Level::INFO, $format, $($arg = $value),*)
    };
}

#[macro_export]
macro_rules! debug {
    (parent: $parent:expr, $format:expr $(,$arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(parent: $parent, tracing::Level::DEBUG, $format, $($arg = $value),*)
    };
    ($format:expr $(, $arg:ident = $value:expr)* $(,)?) => {
        $crate::__export_otel_log_span!(tracing::Level::DEBUG, $format, $($arg = $value),*)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __tracing_span {
    (parent: $parent:expr, $level:expr, $format:expr, $($($arg:ident = $value:expr),+)?) => {{
        // bind args early to avoid multiple evaluation
        $($(let $arg = $value;)*)?
        tracing::span!(
            parent: $parent,
            $level,
            $format,
            $($($arg = $arg,)*)?
            logfire.msg = format_args!($format),
            logfire.json_schema = $crate::__json_schema!($($($arg),+)?),
        )
    }};
    ($level:expr, $format:expr, $($($arg:ident = $value:expr),+)?) => {{
        // bind args early to avoid multiple evaluation
        $($(let $arg = $value;)*)?
        tracing::span!(
            $level,
            $format,
            $($($arg = $arg,)*)?
            logfire.msg = format_args!($format),
            logfire.json_schema = $crate::__json_schema!($($($arg),+)?),
        )
    }};
}

#[doc(hidden)]
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

#[doc(hidden)]
pub struct FallbackToConvertValue<T>(PhantomData<T>);

#[doc(hidden)]
pub struct TryConvertOption<T>(FallbackToConvertValue<T>);

#[doc(hidden)]
pub struct LogfireConverter<T>(TryConvertOption<T>);

#[doc(hidden)]
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

impl<T> std::ops::Deref for LogfireConverter<T> {
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

#[doc(hidden)]
#[expect(clippy::too_many_arguments)] // FIXME probably can group these
pub fn export_log_span(
    name: &'static str,
    parent_span: &tracing::Span,
    message: String,
    level: tracing::Level,
    schema: &'static str,
    file: &'static str,
    line: u32,
    module_path: &'static str,
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
                KeyValue::new("code.filepath", file),
                KeyValue::new("code.lineno", i64::from(line)),
                KeyValue::new("code.namespace", module_path),
                KeyValue::new("thread.id", THREAD_ID.with(|id| *id)),
            ])
            .chain(
                // if thread.name is available, add it
                std::thread::current()
                    .name()
                    .map(|name| KeyValue::new("thread.name", name.to_owned())),
            )
            .collect();

        if !null_args.is_empty() {
            attributes.push(KeyValue::new(
                "logfire.null_args",
                Value::Array(Array::from(null_args)),
            ));
        }

        let ts = SystemTime::now();

        tracer
            .span_builder(name)
            .with_attributes(attributes)
            .with_start_time(ts)
            // .with_end_time(ts) seems to not be respected, need to explicitly end as per below
            .start_with_context(tracer, &parent_span.context())
            .end_with_timestamp(ts);
    });
}

#[macro_export]
#[doc(hidden)]
macro_rules! __export_otel_log_span {
    (parent: $parent:expr, $level:expr, $format:expr, $($($arg:ident = $value:expr),+)?) => {{
        if tracing::span_enabled!($level) {
            // bind args early to avoid multiple evaluation
            $($(let $arg = $value;)*)?
            $crate::export_log_span(
                $format,
                $parent,
                format!($format),
                $level,
                $crate::__json_schema!($($($arg),+)?),
                file!(),
                line!(),
                module_path!(),
                [
                    $($($crate::LogfireValue::new(stringify!($arg), $crate::converter(&$arg).convert_value($arg)),)*)?
                ]
            );
        }
    }};
    ($level:expr, $format:expr, $($($arg:ident = $value:expr),+)?) => {{
        if tracing::span_enabled!($level) {
            // bind args early to avoid multiple evaluation
            $($(let $arg = $value;)*)?
            $crate::export_log_span(
                $format,
                &tracing::Span::current(),
                format!($format),
                $level,
                $crate::__json_schema!($($($arg),+)?),
                file!(),
                line!(),
                module_path!(),
                [
                    $($($crate::LogfireValue::new(stringify!($arg), $crate::converter(&$arg).convert_value($arg)),)*)?
                ]
            );
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __json_schema {
    ($($($args:ident),+)?) => {
        concat!("{\
            \"type\":\"object\",\
            \"properties\":{\
        ",
            $($crate::__schema_args!($($args),*),)?
        "\
            }\
        }")
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __schema_args {
    ($arg:ident, $($args:ident),+) => {
        // this is done recursively to avoid a trailing comma in JSON :/
        concat!($crate::__schema_args!($arg), ",", $crate::__schema_args!($($args),*))
    };
    ($arg:ident) => {
        // TODO proper type analysis for the args
        concat!("\"", stringify!($arg), "\":{}")
    };
    () => {};
}
