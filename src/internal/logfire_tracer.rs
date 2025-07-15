use std::{
    borrow::Cow,
    cell::RefCell,
    sync::{Arc, OnceLock},
    time::SystemTime,
};

use opentelemetry::{
    Array, Value,
    logs::{AnyValue, LogRecord, Logger, Severity},
    trace::TraceContextExt,
};
use opentelemetry_sdk::{logs::SdkLogger, metrics::SdkMeterProvider, trace::Tracer};

use crate::__macros_impl::LogfireValue;

#[derive(Clone)]
pub(crate) struct LogfireTracer {
    pub(crate) inner: Tracer,
    pub(crate) meter_provider: SdkMeterProvider,
    pub(crate) logger: Arc<SdkLogger>,
    pub(crate) handle_panics: bool,
}

// Global tracer configured in `logfire::configure()`
pub(crate) static GLOBAL_TRACER: OnceLock<LogfireTracer> = OnceLock::new();

thread_local! {
    pub(crate) static LOCAL_TRACER: RefCell<Option<LogfireTracer>> = const { RefCell::new(None) };
}

impl LogfireTracer {
    pub(crate) fn try_with<R>(f: impl FnOnce(&LogfireTracer) -> R) -> Option<R> {
        let mut f = Some(f);
        if let Some(result) = LOCAL_TRACER
            .try_with(|local_logfire| {
                local_logfire
                    .borrow()
                    .as_ref()
                    .map(|tracer| f.take().expect("not called")(tracer))
            })
            .ok()
            .flatten()
        {
            return Some(result);
        }

        GLOBAL_TRACER.get().map(f.expect("local tls not used"))
    }

    #[expect(clippy::too_many_arguments)] // FIXME probably can group these
    pub fn export_log(
        &self,
        name: &'static str,
        parent_context: &opentelemetry::Context,
        message: String,
        severity: Severity,
        schema: &'static str,
        file: Option<Cow<'static, str>>,
        line: Option<u32>,
        module_path: Option<Cow<'static, str>>,
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

        let mut null_args: Vec<AnyValue> = Vec::new();

        // Create and emit a log record instead of a span
        let mut log_record = self.logger.create_log_record();

        let ts = SystemTime::now();

        log_record.set_event_name(name);
        log_record.set_timestamp(ts);
        log_record.set_observed_timestamp(ts);
        log_record.set_body(message.clone().into());
        log_record.set_severity_text(severity.name());
        log_record.set_severity_number(severity);

        for arg in args {
            if let Some(value) = arg.value {
                let any_value = match value {
                    Value::Bool(b) => AnyValue::Boolean(b),
                    Value::I64(i) => AnyValue::Int(i),
                    Value::F64(f) => AnyValue::Double(f),
                    Value::String(string_value) => AnyValue::String(string_value),
                    Value::Array(Array::Bool(b)) => {
                        AnyValue::ListAny(Box::new(b.into_iter().map(AnyValue::Boolean).collect()))
                    }
                    Value::Array(Array::I64(i)) => {
                        AnyValue::ListAny(Box::new(i.into_iter().map(AnyValue::Int).collect()))
                    }
                    Value::Array(Array::F64(f)) => {
                        AnyValue::ListAny(Box::new(f.into_iter().map(AnyValue::Double).collect()))
                    }
                    Value::Array(Array::String(s)) => {
                        AnyValue::ListAny(Box::new(s.into_iter().map(AnyValue::String).collect()))
                    }
                    _ => AnyValue::String(format!("{value:?}").into()),
                };
                log_record.add_attribute(arg.name, any_value);
            } else {
                null_args.push(arg.name.as_str().to_owned().into());
            }
        }

        log_record.add_attribute("logfire.msg", message);
        log_record.add_attribute("logfire.json_schema", schema);
        log_record.add_attribute("thread.id", THREAD_ID.with(|id| *id));

        // Add thread name if available
        if let Some(thread_name) = std::thread::current().name() {
            log_record.add_attribute("thread.name", thread_name.to_owned());
        }

        if let Some(file) = file {
            log_record.add_attribute("code.filepath", file);
        }

        if let Some(line) = line {
            log_record.add_attribute("code.lineno", i64::from(line));
        }

        if let Some(module_path) = module_path {
            log_record.add_attribute("code.namespace", module_path);
        }

        if !null_args.is_empty() {
            log_record.add_attribute("logfire.null_args", AnyValue::ListAny(Box::new(null_args)));
        }

        let span = parent_context.span();
        let span_context = span.span_context();
        log_record.set_trace_context(
            span_context.trace_id(),
            span_context.span_id(),
            Some(span_context.trace_flags()),
        );

        self.logger.emit(log_record);
    }
}
